
function saveOptions(e) {
    e.preventDefault();
    const ex = document.querySelector("#ExperimentalLayout").checked;

    browser.storage.sync.set({
        debugStr: document.querySelector("#debugStr").value,
        ExperimentalLayout: ex
    });
}

function restoreOptions() {
    document.querySelector("#debugStr").value = 'Default red';

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
}

document.addEventListener('DOMContentLoaded', restoreOptions);
document.querySelector("form").addEventListener("submit", saveOptions);