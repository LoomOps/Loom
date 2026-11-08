# Managed access resources

Each installation owns a private `admission/managed-resources.json` journal next
to its access-request database. It is not a new server or a public API. It follows
stable Tailnet, machine and device identity, not an OS username or hostname.

Installer policy/tag changes and running approval/revocation changes record their
original and expected state. Reused, unchanged resources are not claimed. An item
that predates Loom but is modified by Loom is restored to its recorded original
value, not deleted. Invitations retain provider/request IDs and whether Loom
created or reused them; the request database remains the delivery-state authority.
No token or full management credential is written to the journal.
Automatic expiration, denial and requester cancellation preserve reused manual
invitations. An explicit Admin revoke still removes the selected share, including
one created outside Loom; that deliberate action is recorded in the request audit.

Writes record intent before a provider mutation, use policy ETags, then record
completion. An interrupted/uncertain response is not silently retried. Inspect
provider state and explicitly reconcile the local receipt without replaying:

```bash
sudo python3 deploy/access_setup.py reconcile-resources --plan /absolute/access-plan.json
```

Reconciliation accepts only a complete before-state or after-state for the pending
change. A mixed/external edit requires operator review. Doctor reports pending
writes and missing journals on newly generated plans. Updates preserve existing
journals. Older installations are not retroactively claimed: newly observed edits
record their real preexisting state; they cannot prove who originally created it.

Uninstall removes owned grants, restores modified entries, removes exclusive tags
and Funnel capabilities and preserves unrelated device tags. Shared Hub tag
definitions and tags still referenced by another service/SSH rule are retained.
Externally modified entries and new group references stop destructive cleanup.
No broad `loom*` matching, Tailnet logout, native-user deletion or device deletion
is performed. Delete the journal only after remote cleanup has been verified.

## Installation files were already removed

Use a separate reviewed policy-only repair. It does not pretend to be a full
uninstall or recover missing invitation history. Select the actual retired device
and exact empty access group. The optional retired IdP selector removes only that
device's obsolete `tailscale.com/cap/tsidp` capability, not its device or SSH rules.

```bash
sudo python3 deploy/policy_cleanup.py plan \
  --credential-file /private/management.json --tailnet example.test \
  --device RETIRED_DEVICE_ID --group group:loom-access-EXACT_SUFFIX \
  --plan /private/orphan-policy-plan.json
# Review the before/after policy and confirm this Loom installation was removed.
sudo python3 deploy/policy_cleanup.py apply \
  --credential-file /private/management.json --plan /private/orphan-policy-plan.json \
  --confirm-removed-device RETIRED_DEVICE_ID
```

Plans are private because they include network policy. Apply checks identity,
recomputes the limited change, rejects a stale policy, validates with the provider
and uses If-Match. It does not auto-retry writes. Repeating an already verified
plan is a no-op. Nonempty groups, broad/extended rules or unrelated references are
not accepted by this orphan-repair path. Keep a private plan/backup outside the
removed instance. User/device invitations, local listeners and host services must
be reconciled separately when their ownership records were lost.

Tailscale does not permit API removal of the final tag from a tag-owned device.
When only Loom-created tags remain and none is needed by a surviving service,
uninstall reports this during preflight, before stopping services. The operator
chooses an appropriate non-Loom machine identity or explicitly reauthenticates as
a user from an alternate/local connection. Uninstall never logs the host out or
claims that retained tags were removed. See the provider's
[tag limitations](https://tailscale.com/docs/features/tags).
