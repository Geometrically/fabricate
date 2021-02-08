use super::ids::*;

pub struct Report {
    pub id: ReportId,
    pub report_type_id: ReportTypeId,
    pub mod_id: Option<ModId>,
    pub version_id: Option<VersionId>,
    pub user_id: Option<UserId>,
    pub body: String,
    pub reporter: UserId,
}

pub struct QueryReport {
    pub id: ReportId,
    pub report_type: String,
    pub mod_id: Option<ModId>,
    pub version_id: Option<VersionId>,
    pub user_id: Option<UserId>,
    pub body: String,
    pub reporter: UserId,
}

impl Report {
    pub async fn insert(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::error::Error> {
        sqlx::query!(
            "
            INSERT INTO reports (
                id, report_type_id, mod_id, version_id, user_id,
                body, reporter
            )
            VALUES (
                $1, $2, $3, $4, $5,
                $6, $7
            )
            ",
            self.id as ReportId,
            self.report_type_id as ReportTypeId,
            self.mod_id.map(|x| x.0 as i64),
            self.version_id.map(|x| x.0 as i64),
            self.user_id.map(|x| x.0 as i64),
            self.body,
            self.reporter as UserId
        )
            .execute(&mut *transaction)
            .await?;

        Ok(())
    }

    pub async fn get<'a, E>(id: ReportId, exec: E) -> Result<Option<QueryReport>, sqlx::Error>
        where
            E: sqlx::Executor<'a, Database = sqlx::Postgres> + Copy,
    {
        let result = sqlx::query!(
            "
            SELECT rt.name, r.mod_id, r.version_id, r.user_id, r.body, r.reporter
            FROM reports r
            INNER JOIN report_types rt ON rt.id = r.report_type_id
            WHERE r.id = $1
            ",
            id as ReportId,
        )
            .fetch_optional(exec)
            .await?;

        if let Some(row) = result {
            Ok(Some(QueryReport {
                id,
                report_type: row.name,
                mod_id: row.mod_id.map(|x| ModId(x)),
                version_id: row.version_id.map(|x| VersionId(x)),
                user_id: row.user_id.map(|x| UserId(x)),
                body: row.body,
                reporter: UserId(row.reporter),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn remove_full<'a, E>(id: ReportId, exec: E) -> Result<Option<()>, sqlx::Error>
        where
            E: sqlx::Executor<'a, Database = sqlx::Postgres> + Copy,
    {
        let result = sqlx::query!(
            "
            SELECT EXISTS(SELECT 1 FROM reports WHERE id = $1)
            ",
            id as ReportId
        )
            .fetch_one(exec)
            .await?;

        if !result.exists.unwrap_or(false) {
            return Ok(None);
        }

        sqlx::query!(
            "
            DELETE FROM reports WHERE id = $1
            ",
            id as ReportId,
        )
            .execute(exec)
            .await?;

        Ok(Some(()))
    }
}