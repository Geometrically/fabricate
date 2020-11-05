use super::ApiError;
use crate::auth::get_user_from_headers;
use crate::database;
use crate::models;
use crate::models::teams::Permissions;
use crate::models::users::Role;
use actix_web::{delete, get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

// TODO: this needs filtering, and a better response type
// Currently it only gives a list of ids, which have to be
// requested manually.  This route could give a list of the
// ids as well as the supported versions and loaders, or
// other info that is needed for selecting the right version.
#[get("version")]
pub async fn version_list(
    info: web::Path<(models::ids::ModId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let id = info.into_inner().0.into();

    let mod_exists = sqlx::query!(
        "SELECT EXISTS(SELECT 1 FROM mods WHERE id = $1)",
        id as database::models::ModId,
    )
    .fetch_one(&**pool)
    .await
    .map_err(|e| ApiError::DatabaseError(e.into()))?
    .exists;

    if mod_exists.unwrap_or(false) {
        let mod_data = database::models::Version::get_mod_versions(id, &**pool)
            .await
            .map_err(|e| ApiError::DatabaseError(e.into()))?;

        let response = mod_data
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<models::ids::VersionId>>();

        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}

#[derive(Serialize, Deserialize)]
pub struct VersionIds {
    pub ids: String,
}

// TODO: Make this return the versions mod struct
#[get("versions")]
pub async fn versions_get(
    web::Query(ids): web::Query<VersionIds>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let version_ids = serde_json::from_str::<Vec<models::ids::VersionId>>(&*ids.ids)?
        .into_iter()
        .map(|x| x.into())
        .collect();
    let versions_data = database::models::Version::get_many(version_ids, &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    use models::mods::VersionType;
    let versions: Vec<models::mods::Version> = versions_data
        .into_iter()
        .map(|data| models::mods::Version {
            id: data.id.into(),
            mod_id: data.mod_id.into(),
            author_id: data.author_id.into(),

            name: data.name,
            version_number: data.version_number,
            changelog_url: data.changelog_url,
            date_published: data.date_published,
            downloads: data.downloads as u32,
            version_type: VersionType::Release,

            files: vec![],
            dependencies: Vec::new(), // TODO: dependencies
            game_versions: vec![],
            loaders: vec![],
        })
        .collect();

    Ok(HttpResponse::Ok().json(versions))
}

#[get("{version_id}")]
pub async fn version_get(
    info: web::Path<(models::ids::VersionId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let id = info.into_inner().0;
    let version_data = database::models::Version::get_full(id.into(), &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    if let Some(data) = version_data {
        use models::mods::VersionType;

        let response = models::mods::Version {
            id: data.id.into(),
            mod_id: data.mod_id.into(),
            author_id: data.author_id.into(),

            name: data.name,
            version_number: data.version_number,
            changelog_url: data.changelog_url,
            date_published: data.date_published,
            downloads: data.downloads as u32,
            version_type: match data.release_channel.as_str() {
                "release" => VersionType::Release,
                "beta" => VersionType::Beta,
                "alpha" => VersionType::Alpha,
                _ => VersionType::Alpha,
            },

            files: data
                .files
                .into_iter()
                .map(|f| {
                    models::mods::VersionFile {
                        url: f.url,
                        filename: f.filename,
                        // FIXME: Hashes are currently stored as an ascii byte slice instead
                        // of as an actual byte array in the database
                        hashes: f
                            .hashes
                            .into_iter()
                            .map(|(k, v)| Some((k, String::from_utf8(v).ok()?)))
                            .collect::<Option<_>>()
                            .unwrap_or_else(Default::default),
                    }
                })
                .collect(),
            dependencies: Vec::new(), // TODO: dependencies
            game_versions: data
                .game_versions
                .into_iter()
                .map(models::mods::GameVersion)
                .collect(),
            loaders: data
                .loaders
                .into_iter()
                .map(models::mods::ModLoader)
                .collect(),
        };
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}

#[delete("{version_id}")]
pub async fn version_delete(
    req: HttpRequest,
    info: web::Path<(models::ids::VersionId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let user = get_user_from_headers(req.headers(), &**pool)
        .await
        .map_err(|_| ApiError::AuthenticationError)?;
    let id = info.into_inner().0;

    if user.role != Role::Moderator || user.role != Role::Admin {
        let version = database::models::Version::get(id.into(), &**pool)
            .await
            .map_err(|e| ApiError::DatabaseError(e.into()))?
            .ok_or_else(|| {
                ApiError::InvalidInputError("Invalid Version ID specified!".to_string())
            })?;
        let mod_item = database::models::Mod::get(version.mod_id, &**pool)
            .await
            .map_err(|e| ApiError::DatabaseError(e.into()))?
            .ok_or_else(|| {
                ApiError::InvalidInputError("Invalid Version ID specified!".to_string())
            })?;
        let team_member = database::models::TeamMember::get_from_user_id(
            mod_item.team_id,
            user.id.into(),
            &**pool,
        )
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?
        .ok_or_else(|| ApiError::InvalidInputError("Invalid Version ID specified!".to_string()))?;

        if Permissions::from_bits_truncate(team_member.permissions as u64).contains(Permissions::DELETE_VERSION) {
            return Err(ApiError::AuthenticationError);
        }
    }

    let result = database::models::Version::remove_full(id.into(), &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    if result.is_some() {
        Ok(HttpResponse::Ok().body(""))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}
