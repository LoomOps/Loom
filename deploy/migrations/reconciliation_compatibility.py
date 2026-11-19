"""Verify the reconciliation authority across native SQLite compatibility boundaries."""
import argparse
import json
import sqlite3
from pathlib import Path


RECONCILIATION_TABLES = {
    "catalog_meta", "reconcile_plans", "asset_imports", "file_bindings",
    "log_retention", "catalog_tasks", "asset_revisions", "asset_locations",
}
JOB_UNIT_TABLES = {"research_jobs", "execution_units", "execution_memberships"}


def verify(instance, phase):
    path = instance / "runtime/ops.sqlite3"
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as db:
        version = db.execute("PRAGMA user_version").fetchone()[0]
        if version in (4, 5, 6) and phase == "check":
            return {"status": "needed"}
        if version not in (7, 8):
            raise ValueError("Unsupported native schema for reconciliation verification")

        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        if not RECONCILIATION_TABLES <= tables or db.execute("PRAGMA integrity_check").fetchall() != [("ok",)]:
            raise ValueError("Reconciliation authority is incomplete")
        if version == 8:
            if not JOB_UNIT_TABLES <= tables:
                raise ValueError("Job and Unit authority is incomplete")
            if db.execute("SELECT minimum_writer FROM rust_authority WHERE id=1").fetchone() != (2,):
                raise ValueError("Job and Unit writer fence is missing")
    return {"status": "done"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=["check", "apply", "verify"])
    parser.add_argument("--instance", type=Path, required=True)
    parser.add_argument("--platform", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.instance, args.phase)))
