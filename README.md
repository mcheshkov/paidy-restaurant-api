# SimpleRestaurantApi assignment from Paidy

Short description of decisions/assumptions made:

* Single restaurant, scaling for this should be easy 
* OTLP kind of load, all request are "narrow" in data sense 
* Low RPS, limited by human operator
* High availability, no SPOF
* Infra is handling multiple app instances and load balancing
* Stateless service, all state shifted to external DB

For more detail refer to [writeup](./writeup.md)
