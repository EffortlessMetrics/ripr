<!-- section: Fixed -->
- A test file swapped for a symlink that points outside the workspace, an
  in-workspace symlink retargeted outside it, or a link further along a
  symlink chain retargeted outside it, is no longer admitted as a repair
  target when the swap keeps the file's size and modification time. The
  file-currentness cache followed symlinks, so it reused its earlier
  "current" answer. It now also records where the path resolves (byte-exact,
  so two non-UTF-8 paths that print alike stay distinct), whether
  each entry is a symlink and where it points, and on Unix the entry's
  device, inode and change time and the followed file's device and inode
  (#5478).
