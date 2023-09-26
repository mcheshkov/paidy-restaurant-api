## Initial assessment

Business case implies that this solution would be used in restaurant, to communicate orders from visitors to kitchen.

This means that all of request-responses would be limited to single restaurant, and even
if we were trying to build something like a platform for a multiple restaurants, each of request-response would
be for a single restaurant.

I would expect that load on this kind of application from every restaurant would not be high: single digits RPS top.
After all, it would be limited by restaurant staff.

We could have a natural way of partitioning load: each restaurant would be a separate "unit" of some sorts, and then
we (as an operator of this system) could move those partitions around for various needs: utilisation, data locality, etc.

Then, all requests require stating table number, and I would assume that operating set for a single
table should be small: how many outstanding items can there be at every moment of time for a single table?
I would assume, that "all items for a table" is compact enough, that we could avoid pagination/..., and just 
put all items in a single response.

There's also something like parties, with huge list, but I still doubt items list would be that large.

I would include high availability in a "production-ready" part of requirements.
Specifically, I would try to avoid single points of failure: failure of this system would mean no way of
communicating new orders.

Some points of failure are out of scope: old school paper based backup solution, tablets connectivity,
tablets themselves. Also client application should be as simple as possible.
I would assume this:
* all tablets have a redundant connection to server
* there are spare tablets, for example in case of HW failure
* information entered in tablet, but not sent to server is non-durable, that case would be covered by staff
* staff would handle cases like "tablet failure between request and response"
* tablets does not have any persistent storage, and is completely volatile

To handle SPOFs in scope:
* Server application should be stateless
* There should be several instances of application on a different hardware/power distribution/ISP/data center/...
* Between client and server there would be load balancer, for all necessary parts of a stack
  * anycast address
  * several L3/4 balancer intances 
  * several L7 balancer instances
* Database would be external to application, and provide HA as well

I also would like to assume for now that necessary infrastructure is already in place: there's a way to deploy app
to several places, to target a load balancer to those places and to launch a database in HA configuration.

Whole application is just a domain-specific DB, so it would be just a simple adapter from our API to DB and back.

What is not clear to me: can I even use external DB? Problem statement says this:

> ... please refrain from using tools which perform API and data structure design for you,
> or hide the data manipulation behind third-party library ...

But DBMS can be seen as a tool to perform data manipulation for me.
E.g. PostgreSQL would layout data to rows, provide snapshots and transactions over them, maintain indices, etc.

Point is even more obvious in case of embeddable DBs, like sqlite/rocks/sled/...

Regarding working set size, I would assume following:
* 1000 tables per restaurant
* Table have short names, useful for staff, so 1KiB per table
* Each item can have comment from staff (like a "no pickles"), 1 KiB should be enough
* Peak RPS for new orders at 100 tables * 100 items per second
* That's 10K * 1KiB ~ 10MiB incoming data
* Sustained load would be much lower
* Total working set will be 1000 table * 1KiB per table + 1000 tables * 100 items * 1KiB per item ~ 101 MiB

Total working set is really low, could easily fit in RAM (at least in context of "server app".
But anyway I want to have no SPOF, so some kind of replicated storage is necessary.

I could use something like embedded Raft with in-memory storage, and hope that all of instances
would never have a power issue. But to simplify, I decided not to.