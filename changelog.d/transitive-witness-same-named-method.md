<!-- section: Fixed -->
- A no-static-path limitation no longer names an unrelated unit test as its
  witness when that test calls a same-named method on another type
  (`Cache::build` while the path runs through `Site::build`). Witnesses that
  call the reaching method on a receiver resolved to its type, or call a free
  function leading to the change, now rank first, so the integration test is
  named and the finding reports `rust_integration_public_api_path_unresolved`
  instead of the generic kind. Classification is unchanged (#5481).
