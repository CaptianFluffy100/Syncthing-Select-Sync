use colored::*;

pub fn ok(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().green(), script.bold().green(), msg.green());
}

pub fn warning(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().yellow(), script.bold().yellow(), msg.yellow());
}

pub fn error(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().red(), script.bold().red(), msg.red());
}

pub fn debug(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic(), script.bold(), msg);
}

pub fn info(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().blue(), script.bold().blue(), msg.blue());
}

pub fn secret(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().purple(), script.bold().purple(), msg.purple());
}

pub fn highlight(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().cyan(), script.bold().cyan(), msg.cyan());
}

pub fn bright(script: &str, msg: &str) {
  println!("[{}] [{}] {}", get_time_stamp().bold().italic().truecolor(255, 0, 139), script.bold().truecolor(255, 0, 139), msg.truecolor(255, 0, 139));
}

fn get_time_stamp() -> String {
  chrono::Local::now().format("%Y-%m-%d %H:%M:%S.%3f %z").to_string()
}