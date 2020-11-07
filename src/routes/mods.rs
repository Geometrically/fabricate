use super::ApiError;
use crate::auth::get_user_from_headers;
use crate::database;
use crate::models;
use crate::models::mods::SearchRequest;
use crate::models::teams::Permissions;
use crate::models::users::Role;
use crate::search::{search_for_mod, SearchConfig, SearchError};
use actix_web::{delete, get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[get("mod")]
pub async fn mod_search(
    web::Query(info): web::Query<SearchRequest>,
    config: web::Data<SearchConfig>,
) -> Result<HttpResponse, SearchError> {
    let results = search_for_mod(&info, &**config).await?;
    Ok(HttpResponse::Ok().json(results))
}

#[derive(Serialize, Deserialize)]
pub struct ModIds {
    pub ids: String,
}

// TODO: Make this return the full mod struct
#[get("mods")]
pub async fn mods_get(
    web::Query(ids): web::Query<ModIds>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let mod_ids = serde_json::from_str::<Vec<models::ids::ModId>>(&*ids.ids)?
        .into_iter()
        .map(|x| x.into())
        .collect();

    let mods_data = database::models::Mod::get_many(mod_ids, &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    let mut mods: Vec<models::mods::Mod> = Vec::new();
    for m in mods_data {
        let status = sqlx::query!(
            "
                SELECT status FROM statuses
                WHERE id = $1
                ",
            m.status.0,
        )
        .fetch_one(&**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?
        .status;

        mods.push(models::mods::Mod {
            id: m.id.into(),
            team: m.team_id.into(),
            title: m.title,
            description: m.description,
            body_url: m.body_url,
            published: m.published,
            updated: m.updated,
            status: models::mods::ModStatus::from_str(&*status),

            downloads: m.downloads as u32,
            categories: vec![],
            versions: vec![],
            icon_url: m.icon_url,
            issues_url: m.issues_url,
            source_url: m.source_url,
            wiki_url: m.wiki_url,
        })
    }

    Ok(HttpResponse::Ok().json(mods))
}

#[get("{id}")]
pub async fn mod_get(
    info: web::Path<(models::ids::ModId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let id = info.into_inner().0;
    let mod_data = database::models::Mod::get_full(id.into(), &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    if let Some(data) = mod_data {
        let m = data.inner;

        let status = sqlx::query!(
            "
            SELECT status FROM statuses
            WHERE id = $1
            ",
            m.status.0,
        )
        .fetch_one(&**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?
        .status;

        let response = models::mods::Mod {
            id: m.id.into(),
            team: m.team_id.into(),
            title: m.title,
            description: m.description,
            body_url: m.body_url,
            published: m.published,
            updated: m.updated,
            status: models::mods::ModStatus::from_str(&*status),

            downloads: m.downloads as u32,
            categories: data.categories,
            versions: data.versions.into_iter().map(|v| v.into()).collect(),
            icon_url: m.icon_url,
            issues_url: m.issues_url,
            source_url: m.source_url,
            wiki_url: m.wiki_url,
        };
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}

#[delete("{id}")]
pub async fn mod_delete(
    req: HttpRequest,
    info: web::Path<(models::ids::ModId,)>,
    pool: web::Data<PgPool>,
    config: web::Data<SearchConfig>,
) -> Result<HttpResponse, ApiError> {
    let user = get_user_from_headers(req.headers(), &**pool)
        .await
        .map_err(|_| ApiError::AuthenticationError)?;
    let id = info.into_inner().0;

    if user.role != Role::Moderator || user.role != Role::Admin {
        let mod_item = database::models::Mod::get(id.into(), &**pool)
            .await
            .map_err(|e| ApiError::DatabaseError(e.into()))?
            .ok_or_else(|| ApiError::InvalidInputError("Invalid Mod ID specified!".to_string()))?;
        let team_member = database::models::TeamMember::get_from_user_id(
            mod_item.team_id,
            user.id.into(),
            &**pool,
        )
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?
        .ok_or_else(|| ApiError::InvalidInputError("Invalid Mod ID specified!".to_string()))?;

        if !team_member.permissions.contains(Permissions::DELETE_MOD) && team_member.accepted {
            return Err(ApiError::AuthenticationError);
        }
    }

    let result = database::models::Mod::remove_full(id.into(), &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    let client = meilisearch_sdk::client::Client::new(&*config.address, &*config.key);

    let indexes: Vec<meilisearch_sdk::indexes::Index> = client.get_indexes().await?;
    for index in indexes {
        index.delete_document(format!("local-{}", id)).await?;
    }

    if result.is_some() {
        Ok(HttpResponse::Ok().body(""))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}
