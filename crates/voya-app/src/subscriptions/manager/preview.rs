use super::{parse::parse_import_text, Result, SubscriptionManager};
use voya_contracts::{ImportPreview, ImportPreviewNode};

impl SubscriptionManager<'_> {
    /// Uses the committing importer parser without opening a write transaction.
    pub fn preview_import(text: &str) -> Result<ImportPreview> {
        let parsed = parse_import_text(text, "")?;
        Ok(ImportPreview {
            nodes: parsed
                .profiles
                .into_iter()
                .map(|profile| ImportPreviewNode {
                    protocol: profile.config_type().as_str().to_string(),
                    address: profile.address().to_string(),
                    name: profile.remarks,
                })
                .collect(),
            subscription_urls: parsed.subscription_urls,
            failed: u32::try_from(parsed.failed_lines).unwrap_or(u32::MAX),
            line_issues: parsed
                .line_issues
                .into_iter()
                .map(crate::contract_map::import_line_issue_to_contract)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_has_no_credentials_and_does_not_require_a_database() {
        let preview = SubscriptionManager::preview_import(
            "trojan://secret@example.test:443#Office\nhttps://sub.example.test/list",
        )
        .unwrap();
        assert_eq!(preview.nodes.len(), 1);
        assert_eq!(preview.nodes[0].address, "example.test");
        assert_eq!(preview.nodes[0].name, "Office");
        assert_eq!(preview.subscription_urls.len(), 1);
        assert_eq!(preview.failed, 0);
    }
}
