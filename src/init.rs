use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{self, Write};

pub fn create_bare_package_lock_json(current_dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let package_lock_json_path = current_dir.join("package-lock.json");
    let mut package_lock_json_file = File::create(&package_lock_json_path)?;
    //get package name from package.json
    if (Path::new("package.json").exists()) {
        let package_json_path = current_dir.join("package.json");
        let package_json_file = File::open(&package_json_path)?;
        let package_json: serde_json::Value = serde_json::from_reader(package_json_file)?;
        let package_name = package_json["name"].as_str().unwrap();
        let package_lock_json: serde_json::Value = serde_json::json!({
            "name": package_name,
            "version": "1.0.0",
            "lockfileVersion": 3,
            "requires": true,
            "dependencies": {}
        });
        package_lock_json_file.write_all(package_lock_json.to_string().as_bytes())?;
        package_lock_json_file.flush()?;
        package_lock_json_file.sync_all()?;
    }
    else
    {
        //get folder name
        let folder_name = current_dir.file_name().unwrap().to_str().unwrap();
        let package_lock_json = serde_json::json!({
            "name": folder_name,
            "version": "1.0.0",
            "lockfileVersion": 3,
            "requires": true,
            "dependencies": {}
        });

        package_lock_json_file.write_all(package_lock_json.to_string().as_bytes())?;
        package_lock_json_file.flush()?;
        package_lock_json_file.sync_all()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        package_lock_json_file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

pub fn create_bare_package_json(current_dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let package_json_path = current_dir.join("package.json");
    let mut package_json_file = File::create(&package_json_path)?;
    package_json_file.write_all(b"{}")?;
    package_json_file.flush()?;
    package_json_file.sync_all()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        package_json_file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

pub fn prompt_with_default(prompt: &str, default: &str) -> String {
    print!("{} (default: {}): ", prompt, default);
    io::stdout().flush().unwrap();  

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => {
            let input = input.trim();
            if input.is_empty() {
                default.to_string()
            } else {
                input.to_string()
            }
        }
        Err(_) => {
            println!("Error reading input. Using default value.");
            default.to_string()
        }
    }
}

pub fn initialize_node(current_dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let folder_name = current_dir.file_name().unwrap().to_str().unwrap();

    let package_name = prompt_with_default("package name", folder_name);
    let version = prompt_with_default("version", "1.0.0");
    let description = prompt_with_default("description", "");
    let entry_point = prompt_with_default("entry point", "index.js");
    let test_command = prompt_with_default("test command", "");
    let git_repository = prompt_with_default("git repository", "");
    let keywords = prompt_with_default("keywords", "");
    let author = prompt_with_default("author", "");
    let license = prompt_with_default("license", "ISC");

    let package_json = serde_json::json!({
        "name": package_name,
        "version": version,
        "description": description,
        "main": entry_point,
        "scripts": {
            "test": test_command
        },
        "repository": {
            "type": "git",
            "url": git_repository
        },
        "keywords": keywords.split(',').map(|s| s.trim()).collect::<Vec<&str>>(),
        "author": author,
        "license": license
    });

    let package_json_path = current_dir.join("package.json");
    let mut package_json_file = File::create(&package_json_path)?;
    package_json_file.write_all(package_json.to_string().as_bytes())?;
    package_json_file.flush()?;
    package_json_file.sync_all()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        package_json_file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}