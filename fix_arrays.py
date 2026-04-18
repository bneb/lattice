s1 = "window.history.pushState({view: 'settings'}, \"\", \"/settings\");\nwindow.addEventListener('popstate', (e) => {\n    if (e.state && e.state.view === 'settings') window._success = true;\n});"
print("script:", len(s1), list(s1.encode("utf-8")))
s2 = "window.history.pushState({view: 'about'}, \"\", \"/about\");"
print("script2:", len(s2), list(s2.encode("utf-8")))
s4 = "if(window._success) {window.history.pushState({}, \"\", \"/validated\");}"
print("script4:", len(s4), list(s4.encode("utf-8")))
