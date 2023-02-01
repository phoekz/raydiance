<info
    title="New user interface"
    link="new-user-interface"
    date="2023-01-15"
    commit="9fb5a3800a337b5d663e1c83932e03fe96abfe0f"
/>

![](images/20230115-140255.webp)

It was time to replace the window title hacks and random keybindings with a real
graphical user interface. We use the the excellent [Dear
ImGui](https://github.com/ocornut/imgui) library. Since our project is written
in Rust, we use [`imgui`](https://crates.io/crates/imgui) and
[`imgui-winit-support`](https://crates.io/crates/imgui-winit-support) crates to
wrap the original C++ library and interface with
[`winit`](https://crates.io/crates/winit).

$$\Lambda(x) = \frac{1}{\pi}$$