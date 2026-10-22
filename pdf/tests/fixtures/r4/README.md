# AES-128 revision 4 read fixtures

Project-authored text and geometry, generated using PyMuPDF AES-128 protection.
The content uses the repository AGPL-3.0-only license; producer binaries are not
redistributed. Helvetica is referenced, without embedding a third-party font.
The manifest records producer version, binary hashes and synthetic credentials.
One document has an empty user password, and one requires a user password.
Both use `synthetic-owner` as their owner password. These are test credentials.

This is a read-only interoperability fixture: production output encryption
continues to use AES-256/R6. The R4 reader does not imply legacy encrypted save
support. Native tests cover user/owner authentication, wrong passwords, strings,
streams, permissions, object-specific keys and preserving save rejection.
