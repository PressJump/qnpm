use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

//run command getting  scripts and running the script associated with arg and running it using node
pub fn run_script(packagejsonpath: &PathBuf, scriptname: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    //read package.json and get scripts
    let packagejson = std::fs::read_to_string(packagejsonpath)?;
    let packagejson: serde_json::Value = serde_json::from_str(&packagejson)?;
    let scripts = packagejson["scripts"].as_object().unwrap();
    let script = scripts.get(scriptname).unwrap().as_str().unwrap();
    let node = Command::new("node")
        .arg("-e")
        .arg(script)
        .output()?;
    println!("{}", String::from_utf8_lossy(&node.stdout));
    Ok(())
}