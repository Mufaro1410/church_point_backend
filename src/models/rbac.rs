use serde::Deserialize;
use uuid::Uuid;

/// What a role can be given. Three actions: `view`, `manage` (create and change; includes view) and
/// `delete` (includes manage). Money records are never deleted, so finance areas have no delete.
pub struct Area {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub delete: bool,
}

pub const AREAS: &[Area] = &[
    Area { key: "members", label: "Members", group: "People", delete: true },
    Area { key: "visitors", label: "Visitors & follow-up", group: "People", delete: true },
    Area { key: "cell_groups", label: "Cell groups & meetings", group: "People", delete: true },
    Area { key: "ministries", label: "Ministries", group: "People", delete: false },
    Area { key: "courses", label: "Courses", group: "People", delete: false },
    Area { key: "events", label: "Events, check-in & serving", group: "Church life", delete: true },
    Area { key: "social_events", label: "Social events", group: "Church life", delete: true },
    Area { key: "announcements", label: "Announcements", group: "Church life", delete: true },
    Area { key: "sermons", label: "Sermons", group: "Church life", delete: true },
    Area { key: "prayers", label: "Prayer requests", group: "Church life", delete: true },
    Area { key: "testimonies", label: "Testimonies", group: "Church life", delete: true },
    Area { key: "moderation", label: "Reported posts", group: "Church life", delete: false },
    Area { key: "dbr", label: "Bible reading plans", group: "Church life", delete: true },
    Area { key: "giving", label: "Giving (tithes, offerings, pledges)", group: "Finance", delete: false },
    Area { key: "expenses", label: "Expenses", group: "Finance", delete: false },
    Area { key: "payments", label: "Online payments", group: "Finance", delete: false },
    Area { key: "finance_settings", label: "Finance setup (currencies, categories, methods)", group: "Finance", delete: false },
    Area { key: "reports", label: "Dashboard & reports", group: "Reports", delete: false },
    Area { key: "settings", label: "Church settings & utilities", group: "Settings", delete: false },
];

pub fn area(key: &str) -> Option<&'static Area> {
    AREAS.iter().find(|a| a.key == key)
}

/// Checks the list, adds what each permission implies (manage → view, delete → manage + view), sorts and dedupes.
pub fn normalise(perms: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for p in perms {
        let (key, action) = p.split_once('.').ok_or_else(|| format!("'{p}' is not a permission."))?;
        let a = area(key).ok_or_else(|| format!("Unknown area '{key}'."))?;
        let implied: &[&str] = match action {
            "view" => &["view"],
            "manage" => &["view", "manage"],
            "delete" if a.delete => &["view", "manage", "delete"],
            _ => return Err(format!("'{p}' is not available.")),
        };
        for i in implied {
            out.push(format!("{key}.{i}"));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    #[serde(default)]
    pub scope_cell_group_ids: Vec<Uuid>,
    #[serde(default)]
    pub scope_age_group_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgeGroupRequest {
    pub name: String,
    pub min_age: i32,
    pub max_age: Option<i32>,
}
