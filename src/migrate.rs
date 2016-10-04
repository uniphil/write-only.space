use crypto::digest::Digest;
use crypto::sha2::Sha256;

use db;


pub fn run(conn: db::PostgresConnection) -> Result<(), ()> {
    println!("migrations");
    let trans = conn.transaction().unwrap();
    let all_migrations =
        [ include_str!("./migrations/init.sql")
        ];
    let all_hashes = all_migrations
        .iter()
        .map(|migration| hash(migration))
        .collect::<Vec<String>>();
    let fresh = trans.execute("
        SELECT *
        FROM information_schema.tables
        WHERE table_name = 'migrations'
        ", &[]).unwrap() == 0;
    if fresh {
        trans.execute("
            CREATE TABLE migrations
            ( migration text PRIMARY KEY
            )", &[]).unwrap();
    }
    let applied = trans.query("
        SELECT migration FROM migrations", &[])
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect::<Vec<String>>();
    for (i, hashed) in applied.iter().enumerate() {
        if !all_hashes.contains(hashed) {
            panic!(format!("Integrity issue: applied migration {} is missing from the migration stack. Past migrations must not be changed or removed once applied.", i));
        }
    }
    for (i, hashed) in all_hashes.iter().enumerate() {
        if applied.contains(hashed) {
            println!("  ✓ {}", i);
        } else {
            println!("  → {} applying...", i);
            trans.batch_execute(all_migrations[i]).unwrap();
            trans.execute("
                INSERT INTO migrations ( migration ) VALUES ( $1 )", &[hashed])
                .unwrap();
        }
    }
    trans.commit().unwrap();
    println!("  done.");
    Ok(())
}


fn hash(s: &&str) -> String {
    let mut hasher = Sha256::new();
    hasher.input_str(s);
    hasher.result_str()
}
