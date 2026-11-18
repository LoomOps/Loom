"""Resolve the same immutable client descriptor used by installation packaging."""
load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

def _client(module_ctx):
    lock = json.decode(module_ctx.read(Label("//loom_use:client-release.json")))
    http_archive(
        name = "loom_use_client",
        urls = [lock["url"]],
        sha256 = lock["sha256"],
        build_file_content = 'exports_files(["VERSION"])\nfilegroup(name="sources",srcs=glob(["**"]),visibility=["//visibility:public"])\n',
    )
client = module_extension(implementation = _client)
