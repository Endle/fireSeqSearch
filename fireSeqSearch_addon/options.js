
function saveOptions(e) {
    e.preventDefault();
    const ex = document.querySelector("#ExperimentalLayout").checked;

    browser.storage.sync.set({
        debugStr: document.querySelector("#debugStr").value,
        ExperimentalLayout: ex,
        ShowScore: document.querySelector("#ShowScore").checked,
        ShowHighlight: document.querySelector("#ShowHighlight").checked
    });
}

function restoreOptions() {
    document.querySelector("#debugStr").value = 'Default red';

    /*global browser */
    let gettingItem = browser.storage.sync.get('debugStr');
    gettingItem.then((res) => {
        document.querySelector("#debugStr").value = res.debugStr || 'Not Found';
    });

    let ex = browser.storage.sync.get('ExperimentalLayout');
    ex.then((res) => {
        if (res.ExperimentalLayout) {
            document.querySelector("#ExperimentalLayout").checked = true;
        }
    });

    browser.storage.sync.get('ShowHighlight')
        .then((res) => {
        if (res.ShowHighlight) {
            document.querySelector("#ShowHighlight").checked = true;
        }
    });
    browser.storage.sync.get('ShowScore')
        .then((res) => {
            if (res.ShowScore) {
                document.querySelector("#ShowScore").checked = true;
            }
        });
}

document.addEventListener('DOMContentLoaded', restoreOptions);
document.querySelector("form").addEventListener("submit", saveOptions);