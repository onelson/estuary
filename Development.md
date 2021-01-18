# Dev Workflow Notes


How to get the test crate published quickly:

```
curl -X PUT  http://localhost:7878/api/v1/crates/new --data-binary "@test_data/publish-my-crate-body"
```

## Running the Procfile

> While the `git daemon` usage has been removed from the project (Estuary is now
> able to handle git over http itself), the `Procfile` remains.
>
> Use it if you like, otherwise just `cargo run` or `cargo watch` however you
> like.

~~During local development you'll want to run both the `estuary` webservice
and `git daemon`, both. This will allow `cargo` to interact with the registry.~~

To simplify this, a `Procfile` has been included. I recommend using [Overmind]
to run it.

Note that since the `web` service in the Procfile is using `cargo watch` to
recompile as your source changes, you'll need to:

```
$ cargo install cargo-watch
```
  
The `Procfile` also anticipates you'll use `_data` as the root path for
development data. This aligns with the environment values given in
`.env.example`, which you should copy as `.env` so `overmind` and `estuary`
can see it.

~~The `.env` file is particularly important since it also contains environment
variables to configure overmind itself, specifically setting the policy for the
`git` service.~~


## Spying on Cargo

One important client we need to support interactions with is `cargo`. It can be
helpful to spy on the HTTP request/response cycle between Cargo and Estuary
during development. My favorite way to do this is by using [mitmproxy].

The most straightforward method for this is to use `mitmproxy` or `mitmweb` in
reverse proxy mode.

Web UI:

```
$ mitmweb --mode=reverse:http://localhost:7878
```

or, Terminal mode:

```
$ mitproxy --mode=reverse:http://localhost:7878
```

With the proxy running, update Estuary's base url to http://localhost:8080
(assuming the default ports).

Lastly, update your cargo registry configuration to point at the proxy instead
of Estuary by adjusting the port in the index url.

Once this is all setup, all the requests and responses travelling between Cargo
and Estuary will be visible in the proxy UI. In addition to being a great source
of feedback, the proxy will offer full request and response bodies for download
(which is a great help for capturing tricky to build payloads like for publishing
crates).

[Overmind]: https://github.com/DarthSim/overmind
[mitmproxy]: https://mitmproxy.org/
