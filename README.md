Mira
====

Small standalone utility to help with Git repository mirroring, using a single
JSON config file. If you read this on Github, I did not push it there myself,
Mira did!



About
-----

Mira automates mirroring a bunch of repository. Some people do not wish to use
Github, Gitlab Cloud or Bitbucket as their primary repository for various
reasons and run their own private Git servers, but making your code available
on these platforms still has advantages (visibility, issues, PR, etc).

Mira is a quick fix to that, hopefully just copy an example below, put it in a
Systemd service and be done with it.



Usage
-----

Example configuration for the mirroring of this repository from a Gitea
instance to its Github mirror:

```json
{
    "workspace": "/tmp/mira",
    "configurations": [
        {
            "name": "Gitea to Github",
            "mirrors": [
                {
		    "name": "Mira",
		    "src": "ssh://git@gitea.example.com:12345/Dece/Mira.git",
		    "dest": "git@github.com:Dece/Mira.git"
                }
            ]
        }
    ]
}
```

The different values are:

- `workspace`: a place where Mira can clone, fetch and push from.
- `configurations`: a set of configurations using similar auth mechanisms.
    For now it is quite useless as no such mechanisms are supported.
- `configurations.N.name`: name of a configuration, a directory in the workspace.
- `configurations.N.mirrors`: list of mirrors.
- `mirrors.N.name`: name of a mirror, used to determine the directory where to
    clone and work from.
- `mirrors.N.src`: clone and fetch URL for the mirror, passed to `git clone`.
    Usually copying the URL provided by your server for cloning should suffice
    (HTTP or SSH).
- `mirrors.N.dest`: push URL for the mirror, set as a remote named "mirror".

It may require some testing to determine what are the appropriate URLs for
clone and push. Gitea with SSH uses the full "ssh://" syntax, whereas Github
uses the simplified scp-like syntax. See the official clone help
[page][git-clone] for details.

[git-clone]: https://git-scm.com/docs/git-clone#_git_urls

When the configuration above is used, the output is the following:

```
$ ./mira -c config.json
Processing config Gitea to Github.
Mira mirrored successfully.
```

It does not support any authentication mechanisms beside what we'll be available
at the shell, which means you should run it in an environment where an SSH agent
will take care of authenticating yourself against the various servers you will
be mirroring from and to.
