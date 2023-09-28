# SimpleRestaurantApi assignment from Paidy

Short description of decisions/assumptions made:

* Single restaurant, scaling for this should be easy 
* OTLP kind of load, all request are "narrow" in data sense 
* Low RPS, limited by human operator
* High availability, no SPOF
* Infra is handling multiple app instances and load balancing
* Stateless service, all state shifted to external DB

For more detail refer to [writeup](./writeup.md)

## Instructions 

`cargo test` to run unit tests. This would _not_ run tests involving external systems (besides OS).

To run PostgreSQL tests start DB server.
You can use `docker-compose up`, or run it explicitly using something like this: 
`podman run -it -e TZ=UTC -e POSTGRES_USER=paidy -e POSTGRES_PASSWORD=paidy -p 5432:5432 -v paidy-restaurant-api_dbdata:/var/lib/postgresql/data --rm docker.io/postgres:16.0`

And then run tests with
`PG_HOST=localhost PG_PORT=5432 PG_USER=paidy PG_PASS=paidy cargo test -- --ignored`

To run load simulator first start PostgreSQL, just as with test, and then run something like this
`POSTGRES_USERNAME=paidy POSTGRES_PASSWORD=paidy RUST_LOG=info cargo run --release -- --postgres-host localhost --postgres-database paidy --postgres-pool 50 --tasks 50`
