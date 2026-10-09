<!-- section: Fixed -->
- Cache: a run terminated between creating and renaming a cache temp file no
  longer leaves a `.ripr-atomic-*.tmp` file behind forever. The next run that
  writes into that cache directory removes such files once they are more than
  10 minutes old, and only files matching the exact name ripr creates; a
  younger file, a directory, a symlink or any other name is left alone.
