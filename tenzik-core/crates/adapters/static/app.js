// Tenzik Receipt Explorer - Frontend Application

let receiptsCache = [];
let expandedReceiptId = null;

// Tab Management
function showTab(tabName) {
    // Hide all tabs
    document.querySelectorAll('.tab-content').forEach(tab => {
        tab.classList.remove('active');
    });
    document.querySelectorAll('.tabs .tab').forEach(tab => {
        tab.classList.remove('active');
    });

    // Show selected tab
    document.getElementById(`${tabName}-tab`).classList.add('active');
    event.target.classList.add('active');
}

// Load receipts on page load
window.addEventListener('DOMContentLoaded', () => {
    refreshReceipts();
    setupSearchFilter();
});

// Refresh receipts from API
async function refreshReceipts() {
    const container = document.getElementById('receipts-list');
    container.innerHTML = '<p class="loading">Loading receipts...</p>';

    try {
        // In a real implementation, this would fetch from /api/receipts
        // For now, we'll show a demo view
        const receipts = await fetchReceipts();
        receiptsCache = receipts;
        displayReceipts(receipts);
    } catch (error) {
        container.innerHTML = `<p class="loading">Error loading receipts: ${error.message}</p>`;
    }
}

// Fetch receipts from API
async function fetchReceipts() {
    try {
        const response = await fetch('/api/receipts');
        if (!response.ok) {
            // If API not available, show demo data
            return getDemoReceipts();
        }
        return await response.json();
    } catch {
        return getDemoReceipts();
    }
}

// Demo receipts for testing
function getDemoReceipts() {
    return [
        {
            receipt_id: "rcpt_1234567890abcdef",
            capsule_hash: "blake3:a1b2c3d4e5f6...",
            timestamp: new Date().toISOString(),
            node_id: "node_demo_001",
            has_zk_proof: true,
            verified: true,
            metrics: {
                gas_used: 12500,
                memory_used: 245000,
                duration_ms: 42,
                capsule_size: 3200
            }
        },
        {
            receipt_id: "rcpt_abcdef1234567890",
            capsule_hash: "blake3:f6e5d4c3b2a1...",
            timestamp: new Date(Date.now() - 3600000).toISOString(),
            node_id: "node_demo_001",
            has_zk_proof: false,
            verified: true,
            metrics: {
                gas_used: 8300,
                memory_used: 180000,
                duration_ms: 28,
                capsule_size: 2800
            }
        }
    ];
}

// Display receipts in the UI
function displayReceipts(receipts) {
    const container = document.getElementById('receipts-list');

    if (receipts.length === 0) {
        container.innerHTML = '<p class="loading">No receipts found. Execute a capsule via the webhook router to generate receipts.</p>';
        return;
    }

    container.innerHTML = receipts.map(receipt => createReceiptCard(receipt)).join('');
}

// Create a receipt card HTML
function createReceiptCard(receipt) {
    const statusClass = receipt.verified ? 'status-verified' : 'status-pending';
    const statusText = receipt.verified ? '✓ Verified' : '⏳ Pending';
    const zkBadge = receipt.has_zk_proof ? '<span class="code">🔐 ZK Proof</span>' : '';

    return `
        <div class="receipt-card" onclick="toggleReceiptDetails('${receipt.receipt_id}')">
            <div class="receipt-header">
                <div class="receipt-id">${receipt.receipt_id}</div>
                <div class="receipt-status ${statusClass}">${statusText}</div>
            </div>

            <div class="receipt-details">
                <div class="detail-item">
                    <div class="detail-label">Capsule Hash</div>
                    <div class="detail-value code">${truncateHash(receipt.capsule_hash)}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Node ID</div>
                    <div class="detail-value">${receipt.node_id}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Timestamp</div>
                    <div class="detail-value">${formatTimestamp(receipt.timestamp)}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Proof Status</div>
                    <div class="detail-value">${zkBadge || '<span class="code">Signature Only</span>'}</div>
                </div>
            </div>

            ${expandedReceiptId === receipt.receipt_id ? createExpandedView(receipt) : ''}
        </div>
    `;
}

// Create expanded receipt view
function createExpandedView(receipt) {
    return `
        <div class="expanded-receipt">
            <h3>Execution Metrics</h3>
            <div class="metric-grid">
                <div class="metric">
                    <div class="metric-value">${formatNumber(receipt.metrics.gas_used)}</div>
                    <div class="metric-label">Gas Used</div>
                </div>
                <div class="metric">
                    <div class="metric-value">${formatBytes(receipt.metrics.memory_used)}</div>
                    <div class="metric-label">Memory</div>
                </div>
                <div class="metric">
                    <div class="metric-value">${receipt.metrics.duration_ms}ms</div>
                    <div class="metric-label">Duration</div>
                </div>
                <div class="metric">
                    <div class="metric-value">${formatBytes(receipt.metrics.capsule_size)}</div>
                    <div class="metric-label">Capsule Size</div>
                </div>
            </div>

            <h3 style="margin-top: 20px;">Full Receipt Data</h3>
            <div class="json-viewer">${formatJSON(receipt)}</div>
        </div>
    `;
}

// Toggle receipt details
function toggleReceiptDetails(receiptId) {
    if (expandedReceiptId === receiptId) {
        expandedReceiptId = null;
    } else {
        expandedReceiptId = receiptId;
    }
    displayReceipts(receiptsCache);
}

// Verify receipt
async function verifyReceipt() {
    const jsonInput = document.getElementById('receipt-json').value.trim();
    const resultBox = document.getElementById('verification-result');

    if (!jsonInput) {
        showResult(resultBox, 'error', 'Please paste a receipt JSON to verify.');
        return;
    }

    try {
        const receipt = JSON.parse(jsonInput);

        // In a real implementation, this would POST to /api/verify
        const response = await fetch('/api/verify', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: jsonInput
        });

        if (response.ok) {
            const result = await response.json();
            showResult(resultBox, 'success',
                `✅ Receipt verified successfully!\n\n` +
                `Signature: ${result.signature_valid ? 'Valid ✓' : 'Invalid ✗'}\n` +
                `ZK Proof: ${result.zk_proof_valid ? 'Valid ✓' : 'Not present'}`
            );
        } else {
            // Demo verification for when API is not available
            demoVerify(resultBox, receipt);
        }
    } catch (error) {
        showResult(resultBox, 'error', `Invalid JSON: ${error.message}`);
    }
}

// Demo verification
function demoVerify(resultBox, receipt) {
    if (receipt.signature && receipt.receipt_id) {
        showResult(resultBox, 'success',
            `✅ Receipt structure valid!\n\n` +
            `Receipt ID: ${receipt.receipt_id}\n` +
            `Signature: Present ✓\n` +
            `ZK Proof: ${receipt.zk_proof ? 'Present ✓' : 'Not attached'}\n\n` +
            `Note: Connect to a running Tenzik node for full cryptographic verification.`
        );
    } else {
        showResult(resultBox, 'error', 'Invalid receipt structure: missing required fields.');
    }
}

// Show verification result
function showResult(box, type, message) {
    box.className = `result-box ${type}`;
    box.textContent = message;
}

// Setup search filter
function setupSearchFilter() {
    const searchInput = document.getElementById('search-receipt');
    searchInput.addEventListener('input', (e) => {
        const query = e.target.value.toLowerCase();
        const filtered = receiptsCache.filter(receipt =>
            receipt.receipt_id.toLowerCase().includes(query) ||
            receipt.capsule_hash.toLowerCase().includes(query) ||
            receipt.node_id.toLowerCase().includes(query)
        );
        displayReceipts(filtered);
    });
}

// Utility functions
function truncateHash(hash) {
    if (hash.length <= 20) return hash;
    return hash.substring(0, 20) + '...';
}

function formatTimestamp(timestamp) {
    const date = new Date(timestamp);
    return date.toLocaleString();
}

function formatNumber(num) {
    return num.toLocaleString();
}

function formatBytes(bytes) {
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

function formatJSON(obj) {
    return JSON.stringify(obj, null, 2)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
}
