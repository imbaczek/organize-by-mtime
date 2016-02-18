organize-by-mtime
=================

Run

```sh
$ organize-by-mtime -h
```

for help.


Example usage
=============

Given a directory like this:

* example/
  * 2013-03-02.jpg
  * .dotfile
  * subdir/
    * 2001-07-14.jpg
    * 2004-12-08.jpg

```sh
$ ls
example/
$ organize-by-mtime --oldest --strip=1 --not-pattern='*~' --not-pattern='.*' --output-dir=output example
move "example/2013-03-02.jpg" "output/2013/2013-03-02.jpg"
move "example/subdir/2001-07-14.jpg" "output/2001/subdir/2001-07-14.jpg"
move "example/subdir/2004-12-08.jpg" "output/2001/subdir/2004-12-08.jpg"
$
```

cf. e.g. --strip=0 (or no strip in other words):

```sh
$ organize-by-mtime --oldest --not-pattern='*~' --not-pattern='.*' --output-dir=output example
move "example/2013-03-02.jpg" "output/2013/example/2013-03-02.jpg"
move "example/subdir/2001-07-14.jpg" "output/2001/example/subdir/2001-07-14.jpg"
move "example/subdir/2004-12-08.jpg" "output/2001/example/subdir/2004-12-08.jpg"
```

Results in a output folder like this:

* output/
    * 2001/
        * subdir/
            * 2001-07-14.jpg
            * 2004-12-08.jpg
    * 2013/
        * 2013-03-02.jpg

Missing directories will be created, but **files will be moved**, so take care! There's a dry run (-d, --dry-run) option, use it to preview changes. **Files will not be overwritten** unless you use --force.

License
=======

MIT, see LICENSE.

Copyright (c) 2016, Marek Baczyñski
