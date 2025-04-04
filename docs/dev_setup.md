# Development Setup

Whether you want to host the index yourself, or contribute to the codebase, this guide will help you get the index running on your machine.

If running on Windows, it is recommended to run the index using the [Windows Subsystem for Linux](https://learn.microsoft.com/en-us/windows/wsl/install).

You will need the following:

- The Rust toolkit, [get it here](https://www.rust-lang.org/learn/get-started)
- PostgreSQL, either installed locally or through a [Docker](https://www.docker.com/) container
- The SQLx CLI, which you can install with `cargo install sqlx-cli`

## 1. Setting up the database

First step after installing all the required tools is setting up your database. If you have installed PostgreSQL locally, you have to setup a new database for the index, alongside a new user. For the purposes of this guide, the database, user and password will all be `geode`.

If you want to run PostgreSQL with Docker, first install Docker for your platform, then you can use the following command in your terminal of choice to run a PostgreSQL container.

```bash
docker run -p 5432:5432 --name=geode-db -v postgres:/var/lib/postgresql/data --restart=unless-stopped -e POSTGRES_DB=geode -e POSTGRES_USER=geode -e POSTGRES_PASSWORD=geode -dit postgres:14-alpine3.20
```

This creates a lightweight container (using Alpine Linux) that contains your database. It exposes the port `5432`, so you can connect to it from outside the container itself. Note that you can change this if you already use `5432` on your machine, just change the first part of the port binding (for example, if I were to use `5433`, my port binding would become `5433:5432`). It also creates a **named volume**, so that the data you enter will be stored between container restarts. The environment variables passed to the container initialize a new database, called `geode`, owned by a new user, called `geode`, with the password `geode`. Easy, right?

You can stop and start your container using `docker stop geode-db`, and `docker start geode-db`, respectively

## 2. Running migrations

Once you have your database setup, we can start configuring the **environment file** of the index. Open a terminal inside your index directory, and run
```bash
cp .env.example .env
```
to create your env file from the given template. Then open the env file with your editor of choice.

The first thing that is recommended is setting `APP_DEBUG` to `1` if you are running the index for development. At the moment, all this does is run the index on one thread only, for easier step debugging.

The second step is setting our `DATABASE_URL`. It has a specific structure: `postgres://{username}:{password}@{db_host}/{database}`. In our case, after completing it with our data, the URL becomes: `postgres://geode:geode@localhost/geode`.

> [!TIP]
> You can get away with setting this up by manually adding developers to the database. Just add columns to `developers` and `auth_tokens` manually, while keeping in mind tokens in the database are sha256'd uuid tokens.

Third, we need to setup a local GitHub OAuth app. Since the index doesn't store passwords, and uses GitHub for logins, we need this step to login into the index. Check out this guide for [creating a GitHub OAuth app](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/creating-an-oauth-app), then fill in the client ID and secret of your app inside the .env file.

Finally, run your migrations from the project directory using `sqlx migrate run`

Next up, set up the log4rs config file found in `config`:
```bash
cp config/log4rs.example.yaml config/log4rs.yaml
```
Feel free to change the settings, but the default works fine.

After all of this is done, you should be able to run `cargo run` inside the index directory. The migrations will be ran automatically, and the index will start. You can check `http://localhost:8000` (if you haven't changed the app port) to see if it all works.

## 3. Admin users

At the moment, there is no easy way to make yourself an admin, other than editing the database itself. A small script to automate this will be created in the future. For now, you can run this simple SQL query:

```sql
UPDATE developers SET admin = true WHERE username = 'YOUR_USERNAME';
```
