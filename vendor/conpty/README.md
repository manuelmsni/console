# ConPTY (Microsoft)

`conpty.dll` and `OpenConsole.exe` from the official NuGet package
[`Microsoft.Windows.Console.ConPTY`](https://www.nuget.org/packages/Microsoft.Windows.Console.ConPTY)
version **1.25.260930003** (from the `runtimes/win-x64/native` and `build/native/runtimes/x64`
folders), signed by Microsoft Corporation and distributed under the MIT license (`LICENSE`).

Console embeds them and uses them instead of the ConPTY built into Windows, which on
Windows 10 drops lines when a program writes more than a screenful at once.

To update: download the new package, replace both files, and change
`BUNDLED_CONPTY_VERSION` in `src/adapters/platform/windows/bundled_conpty.rs`.
