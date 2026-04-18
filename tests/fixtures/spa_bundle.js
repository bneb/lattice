// ============================================================================
// Epic 62: Framework Crucible — Preact-Compatible SPA Bundle
// ============================================================================
// This is a faithful, minimal VDOM implementation that exercises the same
// DOM APIs as Preact: createElement, createTextNode, setAttribute,
// removeAttribute, appendChild, removeChild, insertBefore, addEventListener,
// firstChild, nextSibling, parentNode, nodeType, textContent.
//
// NO ENGINE HACKS. If this throws, the Prisimi DOM implementation is wrong.
// ============================================================================

var h, render, Component;

(function() {
    'use strict';

    // ========================================================================
    // Minimal Virtual DOM Engine (Preact-compatible API surface)
    // ========================================================================

    function VNode(type, props, children) {
        this.type = type;
        this.props = props || {};
        this.children = children || [];
        this._dom = null;
        this._component = null;
    }

    // h(type, props, ...children) — the JSX factory
    h = function(type, props) {
        var children = [];
        for (var i = 2; i < arguments.length; i++) {
            var c = arguments[i];
            if (c == null || c === false || c === true) continue;
            if (typeof c === 'string' || typeof c === 'number') {
                children.push('' + c);
            } else if (Array.isArray(c)) {
                for (var j = 0; j < c.length; j++) children.push(c[j]);
            } else {
                children.push(c);
            }
        }
        return new VNode(type, props || {}, children);
    };

    // Create a real DOM node from a VNode
    function createDOMElement(vnode) {
        if (typeof vnode === 'string') {
            return document.createTextNode(vnode);
        }

        if (typeof vnode.type === 'function') {
            // Component
            var comp = new vnode.type(vnode.props);
            comp._vnode = vnode;
            vnode._component = comp;
            var rendered = comp.render(comp.props, comp.state);
            comp._rendered = rendered;
            var dom = createDOMElement(rendered);
            comp._dom = dom;
            vnode._dom = dom;
            return dom;
        }

        // Plain element
        var el = document.createElement(vnode.type);
        vnode._dom = el;

        // Apply props
        applyProps(el, {}, vnode.props);

        // Append children
        for (var i = 0; i < vnode.children.length; i++) {
            var childDom = createDOMElement(vnode.children[i]);
            el.appendChild(childDom);
        }

        return el;
    }

    // Apply/diff props onto a real DOM element
    function applyProps(el, oldProps, newProps) {
        // Remove old props not in new
        for (var key in oldProps) {
            if (key === 'children') continue;
            if (!(key in newProps)) {
                if (key.substring(0, 2) === 'on') {
                    var evtOld = key.substring(2).toLowerCase();
                    el.removeEventListener(evtOld, oldProps[key]);
                } else if (key === 'style') {
                    el.removeAttribute('style');
                } else if (key === 'value') {
                    // skip
                } else {
                    el.removeAttribute(key);
                }
            }
        }

        // Set new props
        for (var key in newProps) {
            if (key === 'children') continue;
            var val = newProps[key];
            var oldVal = oldProps[key];
            if (val === oldVal) continue;

            if (key.substring(0, 2) === 'on') {
                var evt = key.substring(2).toLowerCase();
                if (oldVal) el.removeEventListener(evt, oldVal);
                if (val) el.addEventListener(evt, val);
            } else if (key === 'style') {
                if (typeof val === 'string') {
                    el.setAttribute('style', val);
                } else if (typeof val === 'object') {
                    // Test both direct assignment and setProperty
                    for (var s in val) {
                        if (s.indexOf('-') > -1 || s.substring(0, 2) === '--') {
                            el.style.setProperty(s, val[s]);
                        } else {
                            el.style[s] = val[s];
                        }
                    }
                }
            } else if (key === 'value') {
                el.setAttribute('value', val);
            } else if (key === 'id') {
                el.id = val;
            } else if (key === 'className') {
                el.className = val;
            } else {
                el.setAttribute(key, val);
            }
        }
    }

    // Diff and patch: oldVNode vs newVNode, updating the real DOM in-place
    function diff(parentDom, oldVNode, newVNode, index) {
        index = index || 0;
        var existingDom = oldVNode ? oldVNode._dom : null;
        if (typeof oldVNode === 'string') {
            var fc = parentDom.firstChild;
            var ci = 0;
            while(fc && ci < index) { fc = fc.nextSibling; ci++; }
            existingDom = fc;
        }

        // New node where there was none
        if (oldVNode == null) {
            var newDom = createDOMElement(newVNode);
            parentDom.appendChild(newDom);
            return;
        }

        // Remove node
        if (newVNode == null) {
            if (existingDom && existingDom.parentNode) {
                existingDom.parentNode.removeChild(existingDom);
            }
            return;
        }

        // Text node diff
        if (typeof oldVNode === 'string' && typeof newVNode === 'string') {
            if (oldVNode !== newVNode) {
                if (existingDom) existingDom.textContent = newVNode;
            }
            return;
        }
        if (typeof newVNode === 'string') {
            var textDom = document.createTextNode(newVNode);
            parentDom.insertBefore(textDom, existingDom);
            if (existingDom) parentDom.removeChild(existingDom);
            return;
        }

        // Component diff
        if (typeof newVNode.type === 'function') {
            var comp = oldVNode._component;
            if (comp && oldVNode.type === newVNode.type) {
                // Re-render same component
                comp.props = newVNode.props;
                newVNode._component = comp;
                var oldRendered = comp._rendered;
                var newRendered = comp.render(comp.props, comp.state);
                diff(parentDom, oldRendered, newRendered);
                comp._rendered = newRendered;
                comp._dom = newRendered._dom || comp._dom;
                newVNode._dom = comp._dom;
            } else {
                // Replace component
                var newDom2 = createDOMElement(newVNode);
                if (existingDom) {
                    parentDom.insertBefore(newDom2, existingDom);
                    parentDom.removeChild(existingDom);
                } else {
                    parentDom.appendChild(newDom2);
                }
            }
            return;
        }

        // Different element type
        if (oldVNode.type !== newVNode.type) {
            var replaceDom = createDOMElement(newVNode);
            parentDom.insertBefore(replaceDom, existingDom);
            if (existingDom) parentDom.removeChild(existingDom);
            return;
        }

        // Same element type — patch in place
        newVNode._dom = existingDom;
        applyProps(existingDom, oldVNode.props, newVNode.props);

        // Diff children
        var oldChildren = oldVNode.children || [];
        var newChildren = newVNode.children || [];
        var maxLen = Math.max(oldChildren.length, newChildren.length);

        for (var i = 0; i < maxLen; i++) {
            var oldChild = i < oldChildren.length ? oldChildren[i] : null;
            var newChild = i < newChildren.length ? newChildren[i] : null;

            if (oldChild == null && newChild != null) {
                // Add new child
                var addDom = createDOMElement(newChild);
                existingDom.appendChild(addDom);
            } else if (newChild == null && oldChild != null) {
                // Remove excess child
                var removeDom = oldChild._dom || oldChild;
                if (typeof removeDom === 'string') {
                    // Text node: find and remove
                    var fc = existingDom.firstChild;
                    var ci = 0;
                    while (fc && ci < i) { fc = fc.nextSibling; ci++; }
                    if (fc) existingDom.removeChild(fc);
                } else if (removeDom && removeDom.parentNode) {
                    removeDom.parentNode.removeChild(removeDom);
                }
            } else {
                diff(existingDom, oldChild, newChild, i);
            }
        }
    }

    // ========================================================================
    // Component base class
    // ========================================================================
    Component = function(props) {
        this.props = props || {};
        this.state = {};
        this._dom = null;
        this._rendered = null;
        this._vnode = null;
        this._parentDom = null;
        this._oldVNode = null;
    };

    Component.prototype.setState = function(update) {
        var newState = {};
        for (var k in this.state) newState[k] = this.state[k];
        if (typeof update === 'function') {
            update = update(newState, this.props);
        }
        for (var k in update) newState[k] = update[k];
        this.state = newState;

        // Re-render: diff old rendered tree vs new rendered tree
        var oldRendered = this._rendered;
        var newRendered = this.render(this.props, this.state);
        
        if (this._dom && this._dom.parentNode) {
            diff(this._dom.parentNode, oldRendered, newRendered);
            this._rendered = newRendered;
            this._dom = newRendered._dom || this._dom;
            if (this._vnode) this._vnode._dom = this._dom;
        }
    };

    // ========================================================================
    // render(vnode, container) — the entry point
    // ========================================================================
    render = function(vnode, container) {
        if (!container) {
            throw new Error('render() target container not found');
        }
        var dom = createDOMElement(vnode);
        container.appendChild(dom);
        
        // Store ref for future re-renders
        if (vnode._component) {
            vnode._component._parentDom = container;
        }
    };

})();

// ============================================================================
// The Application — Counter + Echo (The Crucible Test App)
// ============================================================================

var App = function(props) {
    Component.call(this, props);
    this.state = { count: 0, text: '' };
};
App.prototype = Object.create(Component.prototype);
App.prototype.constructor = App;
App.prototype.render = function(props, state) {
    var self = this;
    return h('div', { id: 'app-container' },
        h('h1', null, 'Prisimi SPA Crucible'),
        h('p', { id: 'count-display' }, 'Count: ' + state.count),
        h('button', { 
            id: 'inc-btn',
            onclick: function() { 
                self.setState({ count: self.state.count + 1 }); 
            }
        }, 'Increment'),
        h('input', {
            id: 'input-box',
            oninput: function(e) {
                var val = '';
                if (e && e.target && e.target.value !== undefined) {
                    val = e.target.value;
                }
                self.setState({ text: val });
            },
            value: state.text
        }),
        h('p', { id: 'echo-display' }, 'Echo: ' + state.text)
    );
};

// ============================================================================
// Mount — The Grand Execution
// ============================================================================
var rootEl = document.getElementById('root');
if (!rootEl) {
    throw new Error('[Framework Crucible] FATAL: <div id="root"> not found in DOM');
}
render(h(App), rootEl);
globalThis.__crucibleMounted = true;
