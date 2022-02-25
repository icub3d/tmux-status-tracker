use std::collections::HashMap;
use std::{error::Error, fs, thread, time};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[clap(author = "Joshua Marsh <joshua@themarshians.com>", version = "1.0", about = "store directory statuses for tmux status bars", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Put (insert or update) a status into the database.
    Put(PutCommand),

    /// Get a status from the database.
    Get(GetCommand),
}

#[derive(Parser, Debug)]
struct PutCommand {
    /// The path of the folder.
    #[clap(short, long)]
    path: String,

    /// The git branch, if any.
    #[clap(short, long, default_value = "")]
    branch: String,

    /// The git status (-sb), if any.
    #[clap(short, long, default_value = "")]
    git_status: String,
}

#[derive(Parser, Debug)]
struct GetCommand {
    /// The path of the folder.
    #[clap(short, long)]
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Status {
    path: String,
    branch: String,
    git_status: HashMap<String, u64>,
}

impl Status {
    fn new(path: &str, branch: &str, git_status: &str) -> Status {
        Status {
            path: path.to_string(),
            branch: branch.to_string(),
            git_status: git_status
                .split('|')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let parts = s.split(' ').collect::<Vec<&str>>();
                    (parts[1].to_string(), parts[0].parse::<u64>().unwrap())
                })
                .collect::<HashMap<String, u64>>(),
        }
    }
}

struct Database {
    db: sled::Db,
}

impl Database {
    fn new(path: &str) -> Result<Database, Box<dyn Error>> {
        let mut attempts = 0;
        loop {
            match sled::open(path) {
                Ok(db) => return Ok(Database { db }),
                Err(e) => {
                    attempts += 1;
                    if attempts > 10 {
                        return Err(format!("failed after 10 attempts: {}", e).into());
                    }
                }
            }
            thread::sleep(time::Duration::from_millis(100))
        }
    }

    fn update(self, status: Status) -> Result<(), Box<dyn std::error::Error>> {
        self.db.insert(
            bincode::serialize(&status.path)?,
            bincode::serialize(&status)?,
        )?;
        self.db.flush()?;
        Ok(())
    }

    fn get(self, path: &str) -> Result<Status, Box<dyn std::error::Error>> {
        Ok(bincode::deserialize(
            &self.db.get(bincode::serialize(path)?)?.unwrap(),
        )?)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create our data directory if it doesn't exist
    let dir = dirs::home_dir()
        .unwrap()
        .join(".config")
        .join("tmux-status-tracker");
    fs::create_dir_all(dir.clone())?;

    let db = Database::new(&dir.into_os_string().into_string().unwrap())?;

    let cli = Cli::parse();
    match &cli.command {
        Commands::Put(p) => {
            let status = Status::new(&p.path, &p.branch, &p.git_status);
            db.update(status)?;
        }
        Commands::Get(g) => {
            let status = db.get(&g.path)?;
            let mut result = status.branch.to_string();
            result += "  ";
            let mut statuses = status.git_status.into_iter().collect::<Vec<_>>();
            statuses.sort_by(|x, y| x.0.cmp(&y.0));
            for (k, v) in statuses.iter() {
                result += &format!("{} {}", v, k);
                result += " ";
            }
            print!("{}", result);
        }
    }
    Ok(())
}
