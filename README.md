# Geode Server

WIP new Geode index, hosted on its own server rather than a Github repository. Uses Actix, SQLite and SQLX in Rust.

## Checklist

- [ ] Mod adding and updating
- [ ] Support for multiple developers per mod
- [ ] Finish the openapi spec
- [ ] Finish the database structure
- [ ] Github OAuth
- [ ] A token system for authenticating the CLI
- [ ] Dependencies

To test, run `setup.sh` to create a test database.

If you want to contribute to this project, please do so! I have no bloody clue how to write a web server.

## Required tools for development

- sqlx-cli
- SQLite

## Setup

> The current database engine used by the Index is SQLite. This is not permanent, we are considering a move to PostgreSQL.

The API uses **sqlx** migrations for setting up your database. To use them in your environment, you need to install the sqlx cli with the following command:

```cargo install sqlx-cli```

Make sure to create an .env file, using the example provided. Remember the path you give in DATABASE_URL.

Make sure you have SQLite installed. To setup your database, run

```sqlite3 db/geode-index.db < db/setup.sql```

After installation, you can run your migrations with ```sqlx migrate run```.
