fn main() {
    // let conn = Connection::open_in_memory()?;
    //
    // // 创建表并插入数据
    // conn.execute_batch("
    //     CREATE TABLE events (
    //         id INTEGER,
    //         event_time TIMESTAMP
    //     );
    //     INSERT INTO events VALUES (1, '2024-01-15 14:30:00');
    // ")?;

    // 查询并映射到 NaiveDateTime
    // let mut stmt = conn.prepare("SELECT event_time FROM events WHERE id = ?")?;
    // let rows = stmt.query_map([1], |row| {
    //     // let dt: NaiveDateTime = row.get(0)?;
    //     let dt = row.get::<_, Option<String>>(0);
    //     dt
    // })?;
    //
    // for dt in rows {
    //     // println!("Datetime: {}", dt.);
    //     match dt? {
    //         Some(t) => println!("{}", t),
    //         None => println!("None"),
    //     };
    //     // println!("Datetime: {}", dt.unwrap().);
    // }
    // Ok(())
    println!("Hello from Rust!");
}