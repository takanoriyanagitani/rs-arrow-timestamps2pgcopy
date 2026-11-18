import { readFile } from "node:fs/promises";
import { PGlite } from "@electric-sql/pglite";

/**
 * Reads stdin into a buffer.
 */
const io_stdin2buffer = () => {
  return readFile("/dev/stdin");
};

/**
 * Converts a buffer to a Blob, as required by PGlite's binary COPY.
 */
const buf2blob = (buf) => new Blob([buf]);

/**
 * Higher-order helper to bind an IO action to a pure transformation.
 */
const io_bind = (iofn, pure) => () => {
  return iofn().then(pure);
};

const io_stdin2blob = io_bind(io_stdin2buffer, buf2blob);

const io_create_table = (db) => () => {
  return db.exec(`
    CREATE TABLE IF NOT EXISTS log(
      id BIGSERIAL PRIMARY KEY,
      timestamp TIMESTAMP,
      timestamp_ms TIMESTAMP,
      timestamp_us TIMESTAMP
    )
  `);
};

/**
 * Performs the binary COPY.
 * The Rust code produces binary data compatible with Postgres TIMESTAMP types.
 */
const io_blob2pg = (blob) => (db) => () => {
  return db.transaction((tx) => {
    return tx.query(
      "COPY log(timestamp, timestamp_ms, timestamp_us) FROM '/dev/blob' WITH (FORMAT BINARY)",
      [],
      { blob },
    );
  });
};

const io_get_log_data = (db) => () => {
  return db.query("SELECT * FROM log ORDER BY id ASC")
    .then((rslt) => rslt?.rows || []);
};

const io_log_results = (rows) => () => {
  console.info(`Imported ${rows.length} rows:`);
  console.table(rows);
  return Promise.resolve();
};

const io_main = () => {
  return Promise.resolve(new PGlite())
    .then((db) => {
      console.info("db initialized.");
      return io_create_table(db)().then(() => db);
    })
    .then((db) => {
      console.info("table created.");
      // Read the binary stream from the Rust process via stdin
      return io_stdin2blob().then((blob) => {
        return io_blob2pg(blob)(db)().then(() => db);
      });
    })
    .then((db) => {
      console.info("copy done");
      return io_get_log_data(db)().then((rows) => {
        return io_log_results(rows)().then(() => db);
      });
    })
    .then((db) => {
      console.info("select done.");
      return db.close();
    });
};

io_main().catch(console.error);
