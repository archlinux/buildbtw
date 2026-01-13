use color_eyre::Result;
use rstest::rstest;

use crate::templates;

/// Test the template engine initialization and parse all templates
#[rstest]
#[tokio::test]
async fn test_templates_initialize() -> Result<()> {
    templates::initialize("./".into()).await
}
