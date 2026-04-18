// Extractor Script to dump DOM Geometry into CSV
// Usage: Paste into Chrome Console on any page, or use via Puppeteer/Playwright

(function extractGeometry() {
    let rows = ["x,y,w,h,tag,id,class"];
    
    // Walk the DOM tree
    function walk(node) {
        if (node.nodeType === Node.ELEMENT_NODE) {
            let rect = node.getBoundingClientRect();
            // Only output visible nodes
            if (rect.width > 0 && rect.height > 0) {
                let x = Math.round(rect.left);
                let y = Math.round(rect.top);
                let w = Math.round(rect.width);
                let h = Math.round(rect.height);
                let tag = node.tagName.toLowerCase();
                let id = node.id || "";
                let cls = (node.className || "").toString().replace(/\\s+/g, ' ').trim();
                
                rows.push(`${x},${y},${w},${h},${tag},${id},${cls}`);
            }
        }
        for (let child of node.childNodes) {
            walk(child);
        }
    }
    
    walk(document.body);
    
    let csvContent = rows.join("\\n");
    
    // Auto-download as CSV
    let blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
    let url = URL.createObjectURL(blob);
    let a = document.createElement("a");
    a.href = url;
    a.download = "chrome_geometry_truth.csv";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    
    console.log("Extracted " + (rows.length - 1) + " visible elements.");
})();
