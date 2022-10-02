import { domainCheck } from "./domaincheck";

function assert(bool: boolean) {
  if (!bool) throw new Error("assert");
}

assert(!domainCheck("foo.com", "bar.com"));
assert(domainCheck("test.foo.com", "www.foo.com"));
assert(domainCheck("test.foo.com", "foo.com"));
assert(!domainCheck("test.fo.com", "foo.com"));
