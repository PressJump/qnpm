use std::error::Error;
use std::path::PathBuf;
use std::fs;
use serde_json::Value;

pub fn add_to_package_json(package_name: &str, current_dir: &PathBuf) -> Result<(), Box<dyn Error + Send + Sync>> {
    let package_json_path = current_dir.join("package.json");
    let mut package_json = std::fs::read_to_string(&package_json_path)?;
    let package_json_value: Value = serde_json::from_str(&package_json)?;
    let mut package_json_object = package_json_value
        .as_object()
        .ok_or("Invalid package.json format")?
        .clone();
    let dependencies = package_json_object
        .entry("dependencies")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(dep_object) = dependencies.as_object_mut() {
        dep_object.insert(
            package_name.to_string(),
            serde_json::Value::String("*".to_string()),
        );
    }
    let updated_json = serde_json::to_string_pretty(&package_json_object)?;
    std::fs::write(package_json_path, updated_json)?;
    Ok(())
}

enum Dependencies {
    Name(String),
    Version(String)
}

// version, resolved, dependencies, engines
pub async fn add_to_package_lock_json(
    package_name: &str, 
    current_dir: &PathBuf, 
    version: &str, 
    resolved: &str, 
    dependencies: Value
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let package_lock_json_path = current_dir.join("package-lock.json");

    let mut package_lock_json = if package_lock_json_path.exists() {
        let package_lock_json_content = fs::read_to_string(&package_lock_json_path)?;
        serde_json::from_str(&package_lock_json_content)?
    } else {
        serde_json::json!({
            "name": package_name,
            "version": "1.0.0",
            "lockfileVersion": 3,
            "requires": true,
            "dependencies": {}
        })
    };

    // Add or update the dependencies
    let dependencies_object = package_lock_json["dependencies"]
        .as_object_mut()
        .ok_or("Invalid package-lock.json format")?;

    dependencies_object.insert(package_name.to_string(), serde_json::json!({
        "version": version,
        "resolved": resolved,
        "dependencies": dependencies
    }));

    let updated_json = serde_json::to_string_pretty(&package_lock_json)?;
    fs::write(package_lock_json_path, updated_json)?;

    Ok(())
}