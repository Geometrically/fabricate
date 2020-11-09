use crate::auth::{check_is_moderator_from_headers, get_user_from_headers};
use crate::database::models::{TeamMember, User};
use crate::models::teams::Permissions;
use crate::models::users::{Role, UserId};
use crate::routes::ApiError;
use actix_web::{delete, get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[get("user")]
pub async fn user_auth_get(
    req: HttpRequest,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::Ok().json(
        get_user_from_headers(
            req.headers(),
            &mut *pool
                .acquire()
                .await
                .map_err(|e| ApiError::DatabaseError(e.into()))?,
        )
        .await?,
    ))
}

#[derive(Serialize, Deserialize)]
pub struct UserIds {
    pub ids: String,
}

#[get("users")]
pub async fn users_get(
    web::Query(ids): web::Query<UserIds>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let user_ids = serde_json::from_str::<Vec<UserId>>(&*ids.ids)?
        .into_iter()
        .map(|x| x.into())
        .collect();

    let users_data = User::get_many(user_ids, &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    let users: Vec<crate::models::users::User> = users_data
        .into_iter()
        .map(|data| crate::models::users::User {
            id: data.id.into(),
            github_id: data.github_id.map(|i| i as u64),
            username: data.username,
            name: data.name,
            email: None,
            avatar_url: data.avatar_url,
            bio: data.bio,
            created: data.created,
            role: Role::from_string(&*data.role),
        })
        .collect();

    Ok(HttpResponse::Ok().json(users))
}

#[get("{id}")]
pub async fn user_get(
    info: web::Path<(UserId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let id = info.into_inner().0;
    let user_data = User::get(id.into(), &**pool)
        .await
        .map_err(|e| ApiError::DatabaseError(e.into()))?;

    if let Some(data) = user_data {
        let response = crate::models::users::User {
            id: data.id.into(),
            github_id: data.github_id.map(|i| i as u64),
            username: data.username,
            name: data.name,
            email: None,
            avatar_url: data.avatar_url,
            bio: data.bio,
            created: data.created,
            role: Role::from_string(&*data.role),
        };
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}

#[get("{user_id}/mods")]
pub async fn mods_list(
    info: web::Path<(UserId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let id = info.into_inner().0.into();

    let user_exists = sqlx::query!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
        id as crate::database::models::UserId,
    )
    .fetch_one(&**pool)
    .await
    .map_err(|e| ApiError::DatabaseError(e.into()))?
    .exists;

    if user_exists.unwrap_or(false) {
        let mod_data = User::get_mods(id, &**pool)
            .await
            .map_err(|e| ApiError::DatabaseError(e.into()))?;

        let response = mod_data
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<crate::models::ids::ModId>>();

        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}

#[get("teams")]
pub async fn teams(
    req: HttpRequest,
    info: web::Path<(UserId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let id: crate::database::models::UserId = info.into_inner().0.into();

    let current_user = get_user_from_headers(req.headers(), &**pool).await.ok();

    let results;
    let mut same_user = false;

    if let Some(user) = current_user {
        if user.id.0 == id.0 as u64 {
            results = TeamMember::get_from_user_private(id, &**pool).await?;
            same_user = true;
        } else {
            results = TeamMember::get_from_user_public(id, &**pool).await?;
        }
    } else {
        results = TeamMember::get_from_user_public(id, &**pool).await?;
    }

    let team_members: Vec<crate::models::teams::TeamMember> = results
        .into_iter()
        .map(|data| crate::models::teams::TeamMember {
            user_id: data.user_id.into(),
            name: data.name,
            role: data.role,
            permissions: if same_user {
                data.permissions
            } else {
                Permissions::default()
            },
        })
        .collect();

    Ok(HttpResponse::Ok().json(team_members))
}

// TODO: Make this actually do stuff
#[delete("{id}")]
pub async fn user_delete(
    req: HttpRequest,
    info: web::Path<(UserId,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    check_is_moderator_from_headers(
        req.headers(),
        &mut *pool
            .acquire()
            .await
            .map_err(|e| ApiError::DatabaseError(e.into()))?,
    )
    .await?;

    let _id = info.0;
    let result = Some(());

    if result.is_some() {
        Ok(HttpResponse::Ok().body(""))
    } else {
        Ok(HttpResponse::NotFound().body(""))
    }
}
