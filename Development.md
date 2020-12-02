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


[Overmind]: https://github.com/DarthSim/overmind
