use std::{fs::File, io::Read, path::Path};

use anyhow::Result;
use serde_yaml::Value;
pub trait ConfigTrait {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self>
    where
        Self: Sized;
    fn sections(&self) -> Result<Value>;
    fn contents(&self) -> Result<String>;
}
pub struct Config {
    sections: String,
}
impl ConfigTrait for Config {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self>
    where
        Self: Sized,
    {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(Config { sections: contents })
    }
    fn sections(&self) -> Result<Value> {
        Ok(serde_yaml::from_str(&self.sections)?)
    }
    fn contents(&self) -> Result<String> {
        Ok(self.sections.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use anyhow::Result;

    use super::{Config, ConfigTrait};

    static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_file_path(test_name: &str) -> PathBuf {
        let file_id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "qa-sys-config-{test_name}-{}-{file_id}.yaml",
            std::process::id(),
        ))
    }

    #[test]
    fn test_should_load_yaml_contents_and_sections() -> Result<()> {
        let path = temporary_file_path("valid");
        let yaml = "service:\n  name: 问答系统\n  port: 50051\n";
        fs::write(&path, yaml)?;

        let config = Config::load(&path)?;
        let sections = config.sections()?;

        assert_eq!(config.contents()?, yaml);
        assert_eq!(sections["service"]["name"], "问答系统");
        assert_eq!(sections["service"]["port"], 50051);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn test_should_return_error_when_config_file_does_not_exist() {
        let path = temporary_file_path("missing");

        assert!(Config::load(path).is_err());
    }

    #[test]
    fn test_should_return_error_for_malformed_yaml_sections() -> Result<()> {
        let path = temporary_file_path("malformed");
        fs::write(&path, "service: [unterminated")?;

        let config = Config::load(&path)?;

        assert!(config.sections().is_err());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn test_should_preserve_empty_config_contents() -> Result<()> {
        let path = temporary_file_path("empty");
        fs::write(&path, "")?;

        let config = Config::load(&path)?;

        assert_eq!(config.contents()?, "");
        fs::remove_file(path)?;
        Ok(())
    }
}
