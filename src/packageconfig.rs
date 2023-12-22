use std::error::Error;
use std::path::PathBuf;
use std::fs;
use serde_json::Value;

enum Dependencies {
    Name(String),
    Version(String)
}

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

pub enum PackageStatus {
    Installed,
    NotInstalled,
    Outdated,
    MalformedDependencies,
    FoundInNodeModules,
    NotFoundInNodeModules,
}

pub fn check_package_status(
    package_name: &str, 
    version: &str, 
    current_dir: &PathBuf
) -> Result<PackageStatus, Box<dyn Error + Send + Sync>> {
    let package_lock_json_path = current_dir.join("package-lock.json");
    let package_json_path = current_dir.join("package.json");

    // =========================================
    // Check if package is installed theoretically or even if not, we check if it is installed in node_modules
    // This is to avoid unnecessary checks, and also user might have installed the package manually
    // Not needed but this will be good for user experience
    // =========================================

    if package_lock_json_path.exists() {
        let package_lock_json = fs::read_to_string(&package_lock_json_path)?;
        if let Ok(status) = check_in_json(&package_lock_json, package_name, version, true) {
            return Ok(status);
        }
    }

    if package_json_path.exists() {
        let package_json = fs::read_to_string(&package_json_path)?;
        if let Ok(status) = check_in_json(&package_json, package_name, version, false) {
            return Ok(status);
        }
    }

    if check_in_node_modules(package_name, current_dir) {
        return Ok(PackageStatus::FoundInNodeModules);
    }

    Ok(PackageStatus::NotFoundInNodeModules)
}

fn check_in_json(
    json_content: &str, 
    package_name: &str, 
    version: &str, 
    is_lock_file: bool
) -> Result<PackageStatus, Box<dyn Error + Send + Sync>> {
    let json_value = serde_json::from_str::<Value>(json_content)
        .map_err(|_| "Invalid JSON format")?;
    let dependencies_key = if is_lock_file { "dependencies" } else { "devDependencies" };
    
    if let Some(Value::Object(dependencies)) = json_value.as_object().and_then(|obj| obj.get(dependencies_key)) {
        return match dependencies.get(package_name).and_then(|v| v.as_str()) {
            Some(installed_version) if installed_version == version => 
                Ok(PackageStatus::Installed),
            Some(_) => Ok(PackageStatus::Outdated),
            None => Ok(PackageStatus::NotInstalled),
        };
    }

    Ok(PackageStatus::MalformedDependencies)
}

pub fn check_in_node_modules(package_name: &str, current_dir: &PathBuf) -> bool {
    let node_modules_path = current_dir.join("node_modules").join(package_name);
    node_modules_path.exists()
}

pub fn remove_from_json(
    json_path: &PathBuf, 
    package_name: &str
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if !json_path.exists() {
        // File does not exist, nothing to remove
        return Ok(());
    }

    let json_content = fs::read_to_string(json_path)?;
    let mut json_value = serde_json::from_str::<Value>(&json_content)
        .map_err(|_| "Invalid JSON format")?;

    if let Some(obj) = json_value.as_object_mut() {
        if let Some(deps) = obj.get_mut("dependencies").and_then(Value::as_object_mut) {
            deps.remove(package_name);
        }
        if let Some(deps) = obj.get_mut("devDependencies").and_then(Value::as_object_mut) {
            deps.remove(package_name);
        }
    }

    let updated_json = serde_json::to_string_pretty(&json_value)?;
    fs::write(json_path, updated_json)?;
    Ok(())
}