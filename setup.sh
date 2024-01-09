#!/bin/sh

# https://stackoverflow.com/questions/29436275/how-to-prompt-for-yes-or-no-in-bash
yes_or_no () {
    while true; do
        read -p "$1 [y/n]: " yn
        case $yn in
            [Yy]*) return 0 ;;  
            [Nn]*) echo "Aborted" ; exit 1 ;;
        esac
    done
}

# Check for old database and delete it if it exists
if test -f "db/geode-index.db"; then
    yes_or_no "Test database already exists, do you want to delete it?"
    echo "Deleting old test database"
    rm db/geode-index.db
fi

# Set up .env if it doesn't exist
if [ ! -f ".env" ]; then
    echo "Setting up .env"
    echo $'DATABASE_URL=sqlite://db/geode-index.db\nPORT=8080' >> .env
fi

# Create new database
echo "Creating new test database"
sqlite3 db/geode-index.db < db/setup.sql

echo "Setup finished"
