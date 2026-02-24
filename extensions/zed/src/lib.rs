use zed_extension_api as zed;

// ------------------------------------------------------
// ## Index
//
// struct KdbZedExtension                             L11
//   fn KdbZedExtension::new()                        L14
//   fn KdbZedExtension::language_server_command()    L18
// ------------------------------------------------------

struct KdbZedExtension;

impl zed::Extension for KdbZedExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != "kdb" {
            return Err(format!(
                "unsupported language server id: {}",
                language_server_id
            ));
        }

        let root = worktree.root_path();

        Ok(zed::Command {
            command: format!("{root}/target/debug/kdb"),
            args: vec!["lsp".to_string(), root],
            env: Vec::new(),
        })
    }
}

zed::register_extension!(KdbZedExtension);
