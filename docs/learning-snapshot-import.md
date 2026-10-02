# Native learning snapshot import boundary

`xhup-cli learning import` accepts only the standard Rime text snapshot named
`xhup_flow_user.userdb.txt`. The filename alone is **not** ownership evidence.

Before invoking native Rime it reads at most 64 MiB, validates UTF-8 and the
standard header, and validates metadata throughout the entire snapshot:

- `/db_name` must be `xhup_flow_user`, or its historical `.userdb`,
  `.userdb.kct`, or `.userdb.txt` spelling. Other identities and paths fail.
- `/db_type` must be `userdb`; `/rime_version` must be a numeric three-component
  version from 1.0.x through 1.16.x. New formats require an explicit compatibility
  review, not optimistic restoration.
- `/tick` is optional, but must be an unsigned integer when present.
- `/user_id` may identify a different installation: cross-install migration is
  intentional, not an ownership failure. Ownership here is dictionary scope,
  not authentication of the person supplying an explicitly selected snapshot.
- Missing required fields, duplicate/unknown metadata, malformed metadata,
  embedded NUL and comment-mode switching fail closed.

Only the validated private staged copy is passed to `rime_dict_manager -r`.
Changing the source after validation cannot retarget the import. Staging uses an
exclusively created directory (0700 on POSIX), a create-new file, and RAII cleanup.
On Windows the temporary directory inherits the current user's temp-directory
ACL. An attacker already running as that same user is outside this boundary.
No external dependency or LevelDB parser was introduced.

Rime's fixed `.temp.userdb` restore workspace must not already exist. This guard
is not a concurrent-operation lock. Native database activity, reset safety and
operation serialization remain the separate F09 work; this change does not claim
to resolve them. Existing native database ownership checks also belong there.

Verification tiers:
- Rust unit tests: malformed/absent/foreign/version/legacy metadata, immutable
  staging and POSIX directory permissions.
- `cargo test -p xhup-cli --test learning_native -- --ignored`: mandatory tool-backed
  integration in the librime CI job. It reproduces `foreign_audit_dictionary`,
  checks no foreign creation and byte-for-byte preservation of an existing real
  foreign DB, restores a historical snapshot, exports the actual restored entry,
  reimports a native export, and protects a preexisting native temporary DB.
- This is **not** target-platform IME/frontend acceptance or a crash test.
