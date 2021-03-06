# Team Heist Tactics

Yayyy!!!!!!!!

For UI specific stuff see ui/

## Developing
Before anything, install the pre-commit hook:

```
cd .git/hooks
ln -s ../../other/pre-commit
```

This makes sure that you format your code before you commit.

First, make sure you have all this stuff:

- Node (>= v14, use nvm to help with this)
- Yarn (https://classic.yarnpkg.com/en/docs/install/)
- Cargo (https://rustup.rs/)
- Rust nightly-2020-06-11 (try `rustup default nightly-2020-06-11`)
- Protoc (http://google.github.io/proto-lens/installing-protoc.html)

Getting all the JS dependencies:
```
cd ui
yarn install
```

Building the UI, generating protobuf types, building the server, and then running it:
```
./run.sh
```
Linting:
```
rg --files | grep '\.rs' | xargs rustfmt --edition 2018
```

**Note**: If you're not using run.sh, make sure to generate the types yourself with `ui/generate_types.sh`, I don't check them in.

## Deploying
Build container:
```
docker build . -t team_heist_tactics
```
Run container:
```
docker run -p 19996:19996 -it team_heist_tactics:latest
```

Note: Running it won't work locally on its own unless you set `THT_DEPLOYMENT_MODE` to `dev` in the Dockerfile.

## Properly deploying
Use https://github.com/banool/server-setup with something like this:
```
ansible-playbook -i hosts_external everything.yaml --extra-vars "@vars.json" --tags base,tht,https,nginx
```
This setup binds a static directory into the container from the host. When the container starts, it copies the static content in to it. Nginx on the host serves the content in there itself.

## Other
I use git lfs to manage the static content right here in the repo instead of using some other static content hosting thingo.
