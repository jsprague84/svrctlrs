/**
 * Job Runs WebSocket Manager
 *
 * Provides real-time updates for the job runs page via WebSocket,
 * replacing HTMX polling for improved efficiency.
 */
class JobRunsWebSocket {
    constructor() {
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000; // Start with 1 second
        this.maxReconnectDelay = 30000; // Max 30 seconds
        this.pingInterval = null;
        this.currentPage = 1;
        this.perPage = 50;
        this.targetElement = null;
        this.isConnected = false;
        this.connectionStatusCallback = null;
    }

    /**
     * Initialize the WebSocket connection
     * @param {string} targetId - The ID of the element to update with job run list
     * @param {function} onConnectionChange - Callback for connection status changes
     */
    init(targetId, onConnectionChange = null) {
        this.targetElement = document.getElementById(targetId);
        this.connectionStatusCallback = onConnectionChange;

        if (!this.targetElement) {
            console.error('JobRunsWebSocket: Target element not found:', targetId);
            return;
        }

        this.connect();
    }

    /**
     * Establish WebSocket connection
     */
    connect() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/job-runs`;

        console.log('JobRunsWebSocket: Connecting to', wsUrl);

        try {
            this.ws = new WebSocket(wsUrl);
            this.setupEventHandlers();
        } catch (error) {
            console.error('JobRunsWebSocket: Connection error:', error);
            this.scheduleReconnect();
        }
    }

    /**
     * Setup WebSocket event handlers
     */
    setupEventHandlers() {
        this.ws.onopen = () => {
            console.log('JobRunsWebSocket: Connected');
            this.isConnected = true;
            this.reconnectAttempts = 0;
            this.reconnectDelay = 1000;

            // Start ping interval
            this.startPing();

            // Notify connection status
            if (this.connectionStatusCallback) {
                this.connectionStatusCallback(true);
            }
        };

        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                this.handleMessage(data);
            } catch (error) {
                console.error('JobRunsWebSocket: Parse error:', error);
            }
        };

        this.ws.onclose = (event) => {
            console.log('JobRunsWebSocket: Disconnected', event.code, event.reason);
            this.isConnected = false;
            this.stopPing();

            // Notify connection status
            if (this.connectionStatusCallback) {
                this.connectionStatusCallback(false);
            }

            // Attempt to reconnect unless intentionally closed
            if (event.code !== 1000) {
                this.scheduleReconnect();
            }
        };

        this.ws.onerror = (error) => {
            console.error('JobRunsWebSocket: Error:', error);
        };
    }

    /**
     * Handle incoming WebSocket messages
     * @param {object} data - Parsed message data
     */
    handleMessage(data) {
        switch (data.type) {
            case 'list':
                // Full list update
                if (data.html && this.targetElement) {
                    this.targetElement.innerHTML = data.html;
                    // Re-initialize Lucide icons for the new content
                    if (window.lucide) {
                        lucide.createIcons({ nameAttr: 'data-lucide', el: this.targetElement });
                    }
                    // Restore any open detail panels
                    if (typeof restoreOpenDetails === 'function') {
                        restoreOpenDetails();
                    }
                }
                break;

            case 'status_changed':
                // Individual status update - update the badge
                if (data.job_run_id && data.status) {
                    const runElement = document.getElementById(`job-run-${data.job_run_id}`);
                    if (runElement) {
                        const badge = runElement.querySelector('.badge');
                        if (badge) {
                            // Update badge class and text
                            badge.className = 'badge ' + this.getStatusBadgeClass(data.status);
                            badge.textContent = this.getStatusText(data.status);
                        }

                        // If status is now complete, request full refresh to get final data
                        if (data.status === 'success' || data.status === 'failed' || data.status === 'cancelled') {
                            this.refresh();
                        }
                    } else {
                        // Job run not visible, request full refresh
                        this.refresh();
                    }
                }
                break;

            case 'pong':
                // Ping response - connection is healthy
                break;

            default:
                console.log('JobRunsWebSocket: Unknown message type:', data.type);
        }
    }

    /**
     * Get badge class for status
     * @param {string} status - Job run status
     * @returns {string} Badge class
     */
    getStatusBadgeClass(status) {
        switch (status) {
            case 'success': return 'badge-success';
            case 'failed': return 'badge-error';
            case 'partial_success': return 'badge-warning';
            case 'running': return 'badge-info';
            case 'cancelled': return 'badge-secondary';
            default: return 'badge-secondary';
        }
    }

    /**
     * Get display text for status
     * @param {string} status - Job run status
     * @returns {string} Status text
     */
    getStatusText(status) {
        switch (status) {
            case 'success': return 'Success';
            case 'failed': return 'Failed';
            case 'partial_success': return 'Partial Success';
            case 'running': return 'Running';
            case 'cancelled': return 'Cancelled';
            default: return 'Pending';
        }
    }

    /**
     * Send a message to the WebSocket server
     * @param {object} message - Message to send
     */
    send(message) {
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        }
    }

    /**
     * Request a manual refresh
     */
    refresh() {
        this.send({
            type: 'refresh',
            page: this.currentPage,
            per_page: this.perPage
        });
    }

    /**
     * Change to a specific page
     * @param {number} page - Page number
     */
    goToPage(page) {
        this.currentPage = page;
        this.send({
            type: 'page',
            page: this.currentPage,
            per_page: this.perPage
        });
    }

    /**
     * Start ping interval
     */
    startPing() {
        this.stopPing();
        this.pingInterval = setInterval(() => {
            if (this.isConnected) {
                this.send({ type: 'ping' });
            }
        }, 30000); // Ping every 30 seconds
    }

    /**
     * Stop ping interval
     */
    stopPing() {
        if (this.pingInterval) {
            clearInterval(this.pingInterval);
            this.pingInterval = null;
        }
    }

    /**
     * Schedule reconnection attempt
     */
    scheduleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.log('JobRunsWebSocket: Max reconnect attempts reached, falling back to HTMX polling');
            // Fallback to HTMX polling by enabling the trigger
            if (this.targetElement) {
                this.targetElement.setAttribute('hx-trigger', 'every 10s');
                htmx.process(this.targetElement);
            }
            return;
        }

        this.reconnectAttempts++;
        const delay = Math.min(this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1), this.maxReconnectDelay);

        console.log(`JobRunsWebSocket: Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);

        setTimeout(() => {
            if (!this.isConnected) {
                this.connect();
            }
        }, delay);
    }

    /**
     * Close the WebSocket connection
     */
    disconnect() {
        this.stopPing();
        if (this.ws) {
            this.ws.close(1000, 'User navigated away');
            this.ws = null;
        }
        this.isConnected = false;
    }
}

// Export singleton instance
window.jobRunsWs = new JobRunsWebSocket();
