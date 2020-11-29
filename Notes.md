General plan is to get a basic registry working that you can publish to using cargo.

Once that it working, look at a way to have the registry passively mirror crates.io
so that you can build even if/when crates.io or github go AWOL (it happens), or
more likely if your internet connection goes down. 

Details:

- <https://doc.rust-lang.org/cargo/reference/registries.html#running-a-registry>
- <https://doc.rust-lang.org/cargo/reference/registries.html#index-format>
- <https://doc.rust-lang.org/cargo/reference/registries.html#web-api>


## Overview of problem space

### Index

- index is represented by git repo
- inside index is
  - json config file with urls used by cargo
    - the `dl` url hosts `.crate` files
    - the `api` url points to the Web API used to alter the index
  - one file per crate
    - each line contains a json object describing 1 version
    - lines in the files should not change (except for the `yanked` field)

Questions:

- can the `dl`/`api` links be paths only (omitting the protocol/host)?
  We'll need a way to modify the index to set the canonical/public urls
  otherwise.
- What's the best way to programmatically manipulate a git repo? Some crate? Shell out?

    
### API

- expect `Authorization` header w/ token (403 if not present or invalid on
  protected endpoints)
- recommended 200 statuses even in the event there are errors (eugh).
    - JSON body with error details in this case can allow cargo to give better
      user feedback.

#### Endpoints

- Publish `PUT /api/v1/crates/new`
- Yank `DELETE /api/v1/crates/{crate_name}/{version}/yank`
- Unyank `PUT /api/v1/crates/{crate_name}/{version}/unyank`
- Owners List `GET /api/v1/crates/{crate_name}/owners`
- Owners Add `PUT /api/v1/crates/{crate_name}/owners`
- Owners Remove `DELETE /api/v1/crates/{crate_name}/owners`
- Search `GET /api/v1/crates`
  - query params: `q` (search terms), `per_page` (result limit - default 10, max 100)
- Login `/me` (no details given re: method; cargo uses this for `cargo login`)

Questions:

- What's the deal with that login endpoint? Need to trial and error, I guess.
- Since owners/ACL is up to the registry (not cargo) do we even want to
  implement this at the start? We could NOOP it.


### Git

We need to be able to act as a "git server" for cargo to interact with us.
This means we need to implement at least the read operations for the
"smart server" protocol detailed in this doc:

- <https://git-scm.com/docs/http-protocol>
- <https://mincong.io/2018/05/04/git-and-http/>
- <https://github.com/dcu/git-http-server>

```
001e# service=git-upload-pack\n
0000
00fafac36d407e123c2499149fcc8c1fc8ebe5ecd301 HEADmulti_ack thin-pack side-band side-band-64k ofs-delta shallow deepen-since deepen-not deepen-relative no-progress include-tag multi_ack_detailed no-done symref=HEAD:refs/heads/master agent=git/2.11.1
003ffac36d407e123c2499149fcc8c1fc8ebe5ecd301 refs/heads/master
0000
```

Looks complicated. I'll revisit this later, but for now I've added a Procfile
that will spawn `git daemon` to handle the git access.

For now this means configuring cargo to talk to this registry will be like:

```
$ cat .cargo/config.toml
[registries]
# Using git protocol to talk to the index repo for now...
estuary = { index = "git://localhost:9418/" }
```