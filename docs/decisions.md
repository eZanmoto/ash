Decisions
=========

About
-----

This document contains information on decisions that were made for this project,
and the justifications for them.

Decisions
---------

### Chain operator

The chain operator has been given a different name and symbol compared to
existing languages, which generally use the name "pipe operator", and the symbol
`|>`.

The symbol was changed to `->` as a matter of preference. The name was changed
to "chain" operator because this project will be handling processes and process
pipelines, where pipes are a core concept, so we avoid overloading this term
with extra meanings.
