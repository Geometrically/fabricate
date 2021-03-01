use super::ids::Base62Id;
use super::users::UserId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Base62Id")]
#[serde(into = "Base62Id")]
pub struct NotificationId(pub u64);

pub struct Notification {
    pub id: NotificationId,
    pub user_id: UserId,
    pub title: String,
    pub text: String,
    pub read: bool,
    pub updated: DateTime<Utc>,
    pub actions: Vec<NotificationAction>,
}

pub struct NotificationAction {
    pub title: String,
    pub action_route: String,
}
