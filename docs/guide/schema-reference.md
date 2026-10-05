# Formation schema reference

## Versions

Locust reads formation schema version 1 only. It refuses other versions and does
not convert them. API and protocol versions are in the
[runtime reference](runtime-reference.md#versions).

## Check a formation against the schema

Download the [JSON Schema](../reference/generated/organization.schema.json) and
the [offline contract](../reference/generated/organization.contract.json), then
run:

```sh
locust formation validate FILE
```

`validate` checks the schema and the rules. On the website, generated tables of
the offline commands, the root fields and the example files follow this text.
