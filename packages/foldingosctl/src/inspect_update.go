package main

import (
	"errors"
	"fmt"
	"os"
)

type inspectStagedUpdate struct {
	CurrentVersion string `json:"current_version"`
	DesiredVersion string `json:"desired_version"`
	ImageSHA256    string `json:"image_sha256"`
	ImageSizeBytes int64  `json:"image_size_bytes"`
	StagedAt       string `json:"staged_at"`
	ApplyState     string `json:"apply_state"`
}

type inspectLastUpdateReport struct {
	ImageVersion string `json:"image_version"`
	Status       string `json:"status"`
	Message      string `json:"message,omitempty"`
	RecordedAt   string `json:"recorded_at"`
}

type inspectUpdateData struct {
	CurrentImageVersion  string                   `json:"current_image_version"`
	DesiredImageVersion  *string                  `json:"desired_image_version,omitempty"`
	DesiredQueryError    *string                  `json:"desired_query_error,omitempty"`
	StagedUpdate         *inspectStagedUpdate     `json:"staged_update,omitempty"`
	LastUpdateReport     *inspectLastUpdateReport `json:"last_update_report,omitempty"`
	RebootRequired       bool                     `json:"reboot_required"`
}

func inspectUpdate() error {
	currentVersion, err := installedFoldingOSVersionReader()
	if err != nil {
		return err
	}
	data := inspectUpdateData{
		CurrentImageVersion: currentVersion,
		RebootRequired:      rebootRequired(),
	}

	if desired, desiredErr := queryDesiredImageVersionForInspect(); desiredErr == nil {
		data.DesiredImageVersion = desired
	} else {
		message := desiredErr.Error()
		data.DesiredQueryError = &message
	}

	if metadata, metaErr := loadStagedUpdateMetadata(); metaErr == nil {
		data.StagedUpdate = &inspectStagedUpdate{
			CurrentVersion: metadata.CurrentVersion,
			DesiredVersion: metadata.DesiredVersion,
			ImageSHA256:    metadata.ImageSHA256,
			ImageSizeBytes: metadata.ImageSizeBytes,
			StagedAt:       metadata.StagedAt,
			ApplyState:     metadata.ApplyState,
		}
	} else if !os.IsNotExist(metaErr) {
		return metaErr
	}

	if report, reportErr := loadPendingUpdateReport(); reportErr == nil {
		data.LastUpdateReport = &inspectLastUpdateReport{
			ImageVersion: report.ImageVersion,
			Status:       report.Status,
			Message:      report.Message,
			RecordedAt:   report.RecordedAt,
		}
	} else if !os.IsNotExist(reportErr) {
		return reportErr
	}

	return automationOrHumanSuccess(data, func() error {
		fmt.Printf("current_image_version=%s reboot_required=%t\n", data.CurrentImageVersion, data.RebootRequired)
		if data.DesiredImageVersion != nil {
			fmt.Printf("desired_image_version=%s\n", *data.DesiredImageVersion)
		}
		if data.StagedUpdate != nil {
			fmt.Printf(
				"staged_update desired=%s apply_state=%s\n",
				data.StagedUpdate.DesiredVersion,
				data.StagedUpdate.ApplyState,
			)
		}
		return nil
	})
}

func queryDesiredImageVersionForInspect() (*string, error) {
	supervisorURL, err := readSupervisorBaseURL()
	if err != nil {
		return nil, err
	}
	if supervisorURL == "" {
		return nil, errors.New("supervisor URL is not configured")
	}
	nodeID, err := readNodeID()
	if err != nil {
		return nil, err
	}
	token, err := readEnrollmentToken()
	if err != nil {
		return nil, err
	}
	desired, err := queryDesiredVersion(supervisorURL, nodeID, token)
	if err != nil {
		return nil, err
	}
	return &desired, nil
}
