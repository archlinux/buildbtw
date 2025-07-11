var cy = cytoscape({
    container: document.getElementById("graph"), // container to render in
    elements: cytoscape_elements,

    layout: {
        name: "cose",
        nodeDimensionsIncludeLabels: true,
    },
    style: [
        {
            selector: "node",
            style: {
                label: "data(label)",
                color: "white",
                "text-outline-color": "black",
                "text-max-width": "50px",
                "text-wrap": "wrap",
                "text-outline-width": "1px",
                "font-size": "20px",
            },
        },
    ],
});
