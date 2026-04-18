// Epic 63: WebComponent Lifecycle Test Payload

class MyCounter extends HTMLElement {
    constructor() {
        super();
        this.count = 0;
        this.innerHTML = "<button id='wc-btn'>WC Count: 0</button>";
    }

    connectedCallback() {
        globalThis.__wcConnected = true;
        let btn = document.getElementById('wc-btn');
        btn.addEventListener('click', () => {
            this.count++;
            btn.textContent = "WC Count: " + this.count;
        });
    }

    disconnectedCallback() {
        globalThis.__wcDisconnected = true;
    }
}

// Emulate HTMLElement for the context
globalThis.HTMLElement = class HTMLElement {};

customElements.define('my-counter', MyCounter);

let container = document.getElementById('root');
globalThis.wc = document.createElement('my-counter');
container.appendChild(globalThis.wc);
globalThis.__wcCreated = true;
