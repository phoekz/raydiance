<info
    title="New user interface"
    link="new-user-interface"
    date="2023-01-15"
    commit="9fb5a3800a337b5d663e1c83932e03fe96abfe0f"
/>

![](media/new-user-interface/title.apng)

It was time to replace the window title hacks and random keybindings with a real
graphical user interface. We use the the excellent [Dear ImGui][imgui-github]
library. Since our project is written in Rust, we use [`imgui`][imgui-crate] and
[`imgui-winit-support`][imgui-winit-support-crate] crates to wrap the original
C++ library and interface with [`winit`][winit-crate].

[imgui-github]: https://github.com/ocornut/imgui
[imgui-crate]: https://crates.io/crates/imgui
[imgui-winit-support-crate]: https://crates.io/crates/imgui-winit-support
[winit-crate]: https://crates.io/crates/winit
