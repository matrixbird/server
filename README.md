### Matrixbird appservice

This appservice powers the backend for [Matrixbird](https://matrixbird.com). 
See the [Matrixbird](https://github.com/matrixbird/matrixbird) page for an
overview of the project.

#### Roadmap

- [x] Route incoming standard email to inbox rooms
- [x] Outgoing standard email
- [x] Verifying remote matrixbird-supported homeservers for federation
- [ ] Sync endpoint for efficient mailbox retrieval
- [ ] End-to-end encryption
- [ ] Possible IMAP/JMAP/standard layer for use with normal clients

#### Running

> [!WARNING]  
> This codebase is experimental, and is not yet ready for use. Run it locally for development, or live with a new matrix homeserver for testing.


Before running this appservice, clone the repository and build it:

```bash
$ git clone https://github.com/matrixbird/server.git
$ cd server
$ cargo build --release
```

Copy the `config.sample.toml` file to `config.toml` and fill in the necessary fields - pointing to your matrix homeserver. It's recommended to use a new and temporary homeserver for now, and not an existing one.

The appservice requires a postgres database to store incoming emails, and transaction events from the matrix homeserver. You'll need the [sqlx-cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md) dependency to setup the database. 

The appservice also needs an admin user to be set up for actions like resetting user passwords etc. This is temporary, until we switch over to native matrix OIC (MAS).

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

The appservice needs to be registered with the homeserver. Refer to Synapse [documentation](https://element-hq.github.io/synapse/latest/application_services.html) for more information on how to do this. Here is a sample registration file:

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

Finally, run the appservice with systemd (or similar), and put it behind a reverse proxy. 

### Discuss

To discuss this project, join the [#matrixbird:matrix.org](https://matrix.to/#/#matrixbird:matrix.org) room.
