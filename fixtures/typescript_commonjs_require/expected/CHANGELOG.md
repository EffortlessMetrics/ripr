# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0085 PR6: new CommonJS require() fixture demonstrating import form extraction from const { x } = require('./path')

Command:
`cargo xtask goldens bless typescript_commonjs_require --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
