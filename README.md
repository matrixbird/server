### Matrixbird appservice

This appservice powers the backend email integration for [Matrixbird](https://github.com/matrixbird/matrixbird). 


Before running it, clone the repository and build the appservice with:

```bash
$ git clone https://github.com/matrixbird/server.git
$ cd server
$ cargo build --release
```

Copy the `config.sample.toml` file to `config.toml` and fill in the necessary fields - pointing to your matrix homeserver etc.

The appservice requires a database to store incoming email data, and transaction events from the matrix homeserver. You'll need the [sqlx-cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md) dependency to setup the database. 

After installing `slqx-cli`:

```bash
$ sqlx database create --database-url=postgres://postgres:postgres@localhost:5432/matrixbird
$ sqlx migrate run
```

Adjust the postgres connection string to match the DB values in `config.toml`.


Run the appservice once with:

```bash
$ ./target/release/matrixbird
```

The URL of the appservice needs to be returned in the homeserver's `.well-known/matrix/client` endpoint, like so:

```json
{
  "m.homeserver": {
    "base_url": "https://matrix.example.com"
  },
  "matrixbird.server": {
    "url": "https://appservice.example.com"
  }
}
```

The appservice needs to be registered with the homeserver. Refer to Synapse for more information on how to do this. Here is a sample registration file:

```yaml
id: "matrixbird"
url: "http://localhost:8999"
as_token: ""
hs_token: ""
sender_localpart: "matrixbird"
rate_limited: false
namespaces:
  rooms:
    - exclusive: false
      regex: "!.*:.*"
  users:
    - exclusive: false
      regex: ".*"
```

Finally, run the appservice with systemd, and put it behind a reverse proxy.


### Discuss

Join the discussion on [Matrix](https://matrix.to/#/#matrixbird:matrix.org).
