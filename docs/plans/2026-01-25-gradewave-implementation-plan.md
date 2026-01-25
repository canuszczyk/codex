# GradeWave Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete remaining GradeWave features including Telnyx SMS integration, bug fixes, and UI consistency improvements.

**Architecture:** .NET 8 backend (C#) with React Native mobile frontend. Uses SendGrid for email, Stripe for billing, PostgreSQL for data storage.

**Tech Stack:** .NET 8, Entity Framework Core, React Native, TypeScript, SendGrid, Stripe, Telnyx (to be added)

**Repository:** `canuszczyk/AIGeneratedGradewave`

---

## Status Summary (Verified 2026-01-25)

| Issue | Title | Status | Notes |
|-------|-------|--------|-------|
| - | Telnyx SMS | **NOT STARTED** | Placeholder stub only |
| #658 | New registration flow | **BACKEND DONE** | Frontend not started |
| #86 | District Admin Classrooms crash | **FIXED** | Can close issue |
| #85 | District Admin filter error | **FIXED** | Can close issue |
| #38 | Reset Password | **PARTIALLY DONE** | Backend exists, frontend placeholder |
| #469 | School admin spelling list | **OPEN** | Shared components exist, not wired |
| #470 | Teacher spelling list | **OPEN** | Shared components exist, not wired |
| #471 | Parent spelling list | **OPEN** | Shared components exist, not wired |
| #421 | District admin user detail | **OPEN** | Adapter exists, not wired |
| #422 | School admin user detail | **OPEN** | Adapter exists, not wired |
| #423 | Principal user detail | **OPEN** | Adapter exists, not wired |
| #88 | PreK/K grade display | **FIXED** | Can close issue |
| #396 | Teacher spelling filters | **PARTIAL** | Needs scope verification |
| #397 | Principal student count | **OPEN** | Backend/data issue |

---

## Issues to Close (Already Fixed)

These issues should be closed on GitHub - the fixes are already in the codebase:

```bash
# Close fixed issues
gh issue close 86 --repo canuszczyk/AIGeneratedGradewave -c "Fixed: districtAdminService.ts now correctly uses response without .data accessor"
gh issue close 85 --repo canuszczyk/AIGeneratedGradewave -c "Fixed: getUsers() method corrected alongside #86 fix"
gh issue close 88 --repo canuszczyk/AIGeneratedGradewave -c "Fixed: MultiLevelSpellingListsManager includes PreK (-1) and K (0) grade options"
```

---

## Task 1: Telnyx SMS Integration (NOT STARTED)

**Priority:** High - Core missing feature

**Current State:**
- `NotificationService.SendSmsInternalAsync()` returns "SMS notifications not implemented"
- Database has `sms_enabled` columns
- UI has SMS toggle switches (non-functional)
- No Telnyx package or integration

**Files:**
- Create: `dotnet-api/src/GradeWave.Infrastructure/Services/TelnyxSmsService.cs`
- Create: `dotnet-api/src/GradeWave.Application/Common/Interfaces/ISmsService.cs`
- Modify: `dotnet-api/src/GradeWave.Infrastructure/Services/NotificationService.cs`
- Modify: `dotnet-api/src/GradeWave.Infrastructure/DependencyInjection.cs`
- Modify: `dotnet-api/src/GradeWave.API/appsettings.json`

### Step 1.1: Add Telnyx NuGet Package

```bash
cd dotnet-api
dotnet add src/GradeWave.Infrastructure/GradeWave.Infrastructure.csproj package Telnyx.net
```

### Step 1.2: Create SMS Service Interface

**File:** `dotnet-api/src/GradeWave.Application/Common/Interfaces/ISmsService.cs`

```csharp
namespace GradeWave.Application.Common.Interfaces;

public interface ISmsService
{
    Task<SmsResult> SendSmsAsync(string toPhoneNumber, string message, CancellationToken cancellationToken = default);
    Task<SmsResult> SendBulkSmsAsync(IEnumerable<string> phoneNumbers, string message, CancellationToken cancellationToken = default);
    Task<bool> ValidatePhoneNumberAsync(string phoneNumber, CancellationToken cancellationToken = default);
}

public record SmsResult(bool Success, string? MessageId, string? ErrorMessage);
```

### Step 1.3: Implement Telnyx SMS Service

**File:** `dotnet-api/src/GradeWave.Infrastructure/Services/TelnyxSmsService.cs`

```csharp
using GradeWave.Application.Common.Interfaces;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Configuration;
using Telnyx;

namespace GradeWave.Infrastructure.Services;

public class TelnyxSmsService : ISmsService
{
    private readonly ILogger<TelnyxSmsService> _logger;
    private readonly string _apiKey;
    private readonly string _fromNumber;
    private readonly string _messagingProfileId;

    public TelnyxSmsService(
        ILogger<TelnyxSmsService> logger,
        IConfiguration configuration)
    {
        _logger = logger;
        _apiKey = configuration["Telnyx:ApiKey"] ?? throw new InvalidOperationException("Telnyx API key not configured");
        _fromNumber = configuration["Telnyx:PhoneNumber"] ?? throw new InvalidOperationException("Telnyx phone number not configured");
        _messagingProfileId = configuration["Telnyx:MessagingProfileId"] ?? string.Empty;

        TelnyxConfiguration.SetApiKey(_apiKey);
    }

    public async Task<SmsResult> SendSmsAsync(string toPhoneNumber, string message, CancellationToken cancellationToken = default)
    {
        try
        {
            var service = new MessagingSenderIdService();
            var options = new NewMessagingSenderId
            {
                From = _fromNumber,
                To = toPhoneNumber,
                Text = message
            };

            if (!string.IsNullOrEmpty(_messagingProfileId))
            {
                options.MessagingProfileId = _messagingProfileId;
            }

            var response = await service.CreateAsync(options, cancellationToken: cancellationToken);

            _logger.LogInformation("SMS sent successfully to {PhoneNumber}, MessageId: {MessageId}",
                toPhoneNumber, response.Id);

            return new SmsResult(true, response.Id?.ToString(), null);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to send SMS to {PhoneNumber}", toPhoneNumber);
            return new SmsResult(false, null, ex.Message);
        }
    }

    public async Task<SmsResult> SendBulkSmsAsync(IEnumerable<string> phoneNumbers, string message, CancellationToken cancellationToken = default)
    {
        var results = new List<SmsResult>();
        foreach (var phoneNumber in phoneNumbers)
        {
            if (cancellationToken.IsCancellationRequested) break;
            var result = await SendSmsAsync(phoneNumber, message, cancellationToken);
            results.Add(result);
            await Task.Delay(100, cancellationToken); // Rate limiting
        }

        var failedCount = results.Count(r => !r.Success);
        return new SmsResult(
            failedCount == 0,
            null,
            failedCount > 0 ? $"{failedCount} of {results.Count} messages failed" : null
        );
    }

    public Task<bool> ValidatePhoneNumberAsync(string phoneNumber, CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(phoneNumber)) return Task.FromResult(false);
        if (!phoneNumber.StartsWith("+")) return Task.FromResult(false);
        var digitsOnly = phoneNumber[1..];
        if (!digitsOnly.All(char.IsDigit)) return Task.FromResult(false);
        if (digitsOnly.Length < 10 || digitsOnly.Length > 15) return Task.FromResult(false);
        return Task.FromResult(true);
    }
}
```

### Step 1.4: Update NotificationService to Use ISmsService

**File:** `dotnet-api/src/GradeWave.Infrastructure/Services/NotificationService.cs`

Replace the stub `SendSmsInternalAsync` method:

```csharp
private readonly ISmsService _smsService;

// Add to constructor
public NotificationService(..., ISmsService smsService)
{
    // ...
    _smsService = smsService;
}

private async Task<NotificationResponse> SendSmsInternalAsync(
    NotificationRequest request,
    string notificationId,
    DateTime createdAt,
    CancellationToken cancellationToken)
{
    if (string.IsNullOrEmpty(request.Recipient.Phone))
    {
        return new NotificationResponse(
            NotificationId: notificationId,
            Type: NotificationType.SMS,
            Status: NotificationStatus.Failed,
            ErrorMessage: "Phone number is required for SMS",
            SentAt: createdAt
        );
    }

    var result = await _smsService.SendSmsAsync(
        request.Recipient.Phone,
        request.Content.Body,
        cancellationToken
    );

    return new NotificationResponse(
        NotificationId: notificationId,
        Type: NotificationType.SMS,
        Status: result.Success ? NotificationStatus.Sent : NotificationStatus.Failed,
        ErrorMessage: result.ErrorMessage,
        SentAt: createdAt,
        ExternalId: result.MessageId
    );
}
```

### Step 1.5: Register Service in DI

**File:** `dotnet-api/src/GradeWave.Infrastructure/DependencyInjection.cs`

```csharp
services.AddScoped<ISmsService, TelnyxSmsService>();
```

### Step 1.6: Add Configuration

**File:** `dotnet-api/src/GradeWave.API/appsettings.json`

```json
{
  "Telnyx": {
    "ApiKey": "",
    "PhoneNumber": "",
    "MessagingProfileId": ""
  }
}
```

### Step 1.7: Commit

```bash
git add -A
git commit -m "feat: implement Telnyx SMS integration

- Add ISmsService interface and TelnyxSmsService implementation
- Update NotificationService to use real SMS sending
- Register SMS service in dependency injection
- Add Telnyx configuration section

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 2: Complete Reset Password (#38)

**Priority:** High

**Current State:**
- Backend: `AdminUserController.ResetPassword()` exists but only generates temp password, no email
- Frontend: `ParentDetailScreen.tsx` shows "Feature Coming Soon" placeholder

**Files:**
- Modify: `dotnet-api/src/GradeWave.API/Controllers/AdminUserController.cs`
- Modify: `mobile-apps/parent-dashboard/src/screens/districtAdmin/users/ParentDetailScreen.tsx`
- Modify: `mobile-apps/parent-dashboard/src/screens/shared/users/detail/adapters/districtUserManagementAdapter.tsx`

### Step 2.1: Update Backend to Send Email

**File:** `dotnet-api/src/GradeWave.API/Controllers/AdminUserController.cs`

Modify the `ResetPassword` method to send email with temporary password:

```csharp
[HttpPost("users/{id}/reset-password")]
public async Task<IActionResult> ResetPassword(int id)
{
    var user = await _userRepository.GetByIdAsync(id);
    if (user == null)
        return NotFound(new { message = "User not found" });

    var temporaryPassword = GenerateTemporaryPassword();
    user.password_hash = BCrypt.Net.BCrypt.HashPassword(temporaryPassword);
    user.must_change_password = true;
    await _userRepository.UpdateAsync(user);

    // Send email with temporary password
    await _emailService.SendAsync(new EmailMessage
    {
        To = user.email,
        Subject = "GradeWave Password Reset",
        Body = $"Your temporary password is: {temporaryPassword}\n\nPlease log in and change your password immediately."
    });

    return Ok(new {
        message = "Password reset email sent successfully",
        email = user.email
    });
}
```

### Step 2.2: Add Service Method to Adapter

**File:** `mobile-apps/parent-dashboard/src/screens/shared/users/detail/adapters/districtUserManagementAdapter.tsx`

Add resetPassword action:

```typescript
resetPassword: async (userId: number) => {
  const response = await apiService.post(`/admin/users/${userId}/reset-password`);
  return response;
},
```

### Step 2.3: Update Frontend Handler

**File:** `mobile-apps/parent-dashboard/src/screens/districtAdmin/users/ParentDetailScreen.tsx`

Replace placeholder:

```typescript
const handleResetPassword = async () => {
  try {
    setIsLoading(true);
    await districtAdminService.resetUserPassword(user.id);
    Alert.alert('Success', `Password reset email sent to ${user.email}`);
  } catch (error) {
    Alert.alert('Error', 'Failed to send password reset email');
  } finally {
    setIsLoading(false);
  }
};
```

### Step 2.4: Commit

```bash
git add -A
git commit -m "feat: complete password reset with email notification

- Backend now sends email with temporary password
- Frontend calls API instead of showing placeholder
- Add resetPassword to district user management adapter
Fixes #38.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Complete New Registration Flow Frontend (#658)

**Priority:** High

**Current State:**
- Backend: COMPLETE (DTOs, entities, service, controller, migration all done)
- Frontend: NOT STARTED (no InviteAcceptanceScreen, no InviteParentModal)

**Files to Create:**
- `mobile-apps/parent-dashboard/src/services/invitationService.ts`
- `mobile-apps/parent-dashboard/src/screens/auth/InviteAcceptanceScreen.tsx`
- `mobile-apps/parent-dashboard/src/screens/schoolAdmin/parents/InviteParentModal.tsx`

**Files to Modify:**
- `mobile-apps/parent-dashboard/src/navigation/AppNavigator.tsx`
- `mobile-apps/parent-dashboard/src/screens/schoolAdmin/parents/ParentListScreenV2.tsx`

### Step 3.1: Create Invitation Service

**File:** `mobile-apps/parent-dashboard/src/services/invitationService.ts`

```typescript
import apiService from './api';

export interface CreateInvitationRequest {
  email: string;
  studentUserIds: number[];
  customMessage?: string;
  schoolId: number;
}

export interface InvitationResponse {
  id: number;
  email: string;
  status: string;
  expiresAt: string;
  inviterName: string;
}

export interface ValidateInvitationResponse {
  isValid: boolean;
  invitation?: {
    email: string;
    inviterName: string;
    schoolName: string;
    students: { id: number; firstName: string; lastName: string }[];
  };
  errorMessage?: string;
}

export interface AcceptInvitationRequest {
  firstName: string;
  lastName: string;
  password: string;
  phoneNumber?: string;
}

const invitationService = {
  async createInvitation(request: CreateInvitationRequest): Promise<InvitationResponse> {
    return apiService.post('/parent-invitations', request);
  },

  async validateToken(token: string): Promise<ValidateInvitationResponse> {
    return apiService.get(`/parent-invitations/validate/${token}`);
  },

  async acceptInvitation(token: string, request: AcceptInvitationRequest): Promise<{ success: boolean }> {
    return apiService.post(`/parent-invitations/accept/${token}`, request);
  },

  async getSentInvitations(): Promise<InvitationResponse[]> {
    return apiService.get('/parent-invitations/sent');
  },

  async revokeInvitation(id: number): Promise<void> {
    return apiService.delete(`/parent-invitations/${id}`);
  },

  async resendInvitation(id: number): Promise<void> {
    return apiService.post(`/parent-invitations/${id}/resend`);
  },
};

export default invitationService;
```

### Step 3.2: Create InviteAcceptanceScreen

**File:** `mobile-apps/parent-dashboard/src/screens/auth/InviteAcceptanceScreen.tsx`

```typescript
import React, { useState, useEffect } from 'react';
import { View, Text, TextInput, TouchableOpacity, Alert, ActivityIndicator, StyleSheet } from 'react-native';
import invitationService, { ValidateInvitationResponse, AcceptInvitationRequest } from '../../services/invitationService';

interface Props {
  route: { params: { token: string } };
  navigation: any;
}

export default function InviteAcceptanceScreen({ route, navigation }: Props) {
  const { token } = route.params;
  const [loading, setLoading] = useState(true);
  const [invitation, setInvitation] = useState<ValidateInvitationResponse['invitation']>(null);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<AcceptInvitationRequest>({
    firstName: '',
    lastName: '',
    password: '',
    phoneNumber: '',
  });
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    validateInvitation();
  }, [token]);

  const validateInvitation = async () => {
    try {
      const response = await invitationService.validateToken(token);
      if (response.isValid && response.invitation) {
        setInvitation(response.invitation);
      } else {
        setError(response.errorMessage || 'Invalid or expired invitation');
      }
    } catch (err) {
      setError('Failed to validate invitation');
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async () => {
    if (!form.firstName || !form.lastName || !form.password) {
      Alert.alert('Error', 'Please fill in all required fields');
      return;
    }
    if (form.password.length < 8) {
      Alert.alert('Error', 'Password must be at least 8 characters');
      return;
    }

    setSubmitting(true);
    try {
      await invitationService.acceptInvitation(token, form);
      Alert.alert('Success', 'Account created! You can now log in.', [
        { text: 'OK', onPress: () => navigation.navigate('Login') }
      ]);
    } catch (err) {
      Alert.alert('Error', 'Failed to create account. Please try again.');
    } finally {
      setSubmitting(false);
    }
  };

  if (loading) {
    return <ActivityIndicator size="large" style={styles.loader} />;
  }

  if (error) {
    return (
      <View style={styles.container}>
        <Text style={styles.errorText}>{error}</Text>
        <TouchableOpacity onPress={() => navigation.navigate('Login')}>
          <Text style={styles.link}>Return to Login</Text>
        </TouchableOpacity>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Accept Invitation</Text>
      <Text style={styles.subtitle}>
        You've been invited by {invitation?.inviterName} to join {invitation?.schoolName}
      </Text>

      {invitation?.students && invitation.students.length > 0 && (
        <View style={styles.studentList}>
          <Text style={styles.label}>Students:</Text>
          {invitation.students.map(s => (
            <Text key={s.id} style={styles.student}>{s.firstName} {s.lastName}</Text>
          ))}
        </View>
      )}

      <TextInput
        style={styles.input}
        placeholder="First Name *"
        value={form.firstName}
        onChangeText={(v) => setForm({ ...form, firstName: v })}
      />
      <TextInput
        style={styles.input}
        placeholder="Last Name *"
        value={form.lastName}
        onChangeText={(v) => setForm({ ...form, lastName: v })}
      />
      <TextInput
        style={styles.input}
        placeholder="Password *"
        secureTextEntry
        value={form.password}
        onChangeText={(v) => setForm({ ...form, password: v })}
      />
      <TextInput
        style={styles.input}
        placeholder="Phone Number (optional)"
        value={form.phoneNumber}
        onChangeText={(v) => setForm({ ...form, phoneNumber: v })}
      />

      <TouchableOpacity
        style={[styles.button, submitting && styles.buttonDisabled]}
        onPress={handleSubmit}
        disabled={submitting}
      >
        <Text style={styles.buttonText}>
          {submitting ? 'Creating Account...' : 'Create Account'}
        </Text>
      </TouchableOpacity>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, padding: 20, justifyContent: 'center' },
  loader: { flex: 1, justifyContent: 'center' },
  title: { fontSize: 24, fontWeight: 'bold', marginBottom: 10 },
  subtitle: { fontSize: 16, color: '#666', marginBottom: 20 },
  studentList: { marginBottom: 20, padding: 10, backgroundColor: '#f5f5f5', borderRadius: 8 },
  label: { fontWeight: 'bold', marginBottom: 5 },
  student: { marginLeft: 10 },
  input: { borderWidth: 1, borderColor: '#ddd', borderRadius: 8, padding: 12, marginBottom: 15 },
  button: { backgroundColor: '#007AFF', padding: 15, borderRadius: 8, alignItems: 'center' },
  buttonDisabled: { opacity: 0.6 },
  buttonText: { color: 'white', fontWeight: 'bold', fontSize: 16 },
  errorText: { fontSize: 18, color: '#d32f2f', textAlign: 'center', marginBottom: 20 },
  link: { color: '#007AFF', textAlign: 'center' },
});
```

### Step 3.3: Create InviteParentModal

**File:** `mobile-apps/parent-dashboard/src/screens/schoolAdmin/parents/InviteParentModal.tsx`

```typescript
import React, { useState } from 'react';
import { View, Text, TextInput, TouchableOpacity, Modal, Alert, StyleSheet } from 'react-native';
import invitationService from '../../../services/invitationService';

interface Props {
  visible: boolean;
  onClose: () => void;
  schoolId: number;
  students: { id: number; name: string }[];
  onSuccess: () => void;
}

export default function InviteParentModal({ visible, onClose, schoolId, students, onSuccess }: Props) {
  const [email, setEmail] = useState('');
  const [selectedStudents, setSelectedStudents] = useState<number[]>([]);
  const [customMessage, setCustomMessage] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const toggleStudent = (id: number) => {
    setSelectedStudents(prev =>
      prev.includes(id) ? prev.filter(s => s !== id) : [...prev, id]
    );
  };

  const handleSubmit = async () => {
    if (!email || !email.includes('@')) {
      Alert.alert('Error', 'Please enter a valid email address');
      return;
    }
    if (selectedStudents.length === 0) {
      Alert.alert('Error', 'Please select at least one student');
      return;
    }

    setSubmitting(true);
    try {
      await invitationService.createInvitation({
        email,
        studentUserIds: selectedStudents,
        customMessage: customMessage || undefined,
        schoolId,
      });
      Alert.alert('Success', 'Invitation sent!');
      setEmail('');
      setSelectedStudents([]);
      setCustomMessage('');
      onSuccess();
      onClose();
    } catch (err: any) {
      if (err?.response?.status === 409) {
        Alert.alert('Parent Exists', 'This email is already registered. Would you like to link them instead?');
      } else {
        Alert.alert('Error', 'Failed to send invitation');
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal visible={visible} animationType="slide" transparent>
      <View style={styles.overlay}>
        <View style={styles.modal}>
          <Text style={styles.title}>Invite Parent</Text>

          <TextInput
            style={styles.input}
            placeholder="Parent Email"
            value={email}
            onChangeText={setEmail}
            keyboardType="email-address"
            autoCapitalize="none"
          />

          <Text style={styles.label}>Select Students:</Text>
          <View style={styles.studentList}>
            {students.map(s => (
              <TouchableOpacity
                key={s.id}
                style={[styles.studentItem, selectedStudents.includes(s.id) && styles.studentSelected]}
                onPress={() => toggleStudent(s.id)}
              >
                <Text>{s.name}</Text>
              </TouchableOpacity>
            ))}
          </View>

          <TextInput
            style={[styles.input, styles.messageInput]}
            placeholder="Custom message (optional)"
            value={customMessage}
            onChangeText={setCustomMessage}
            multiline
          />

          <View style={styles.buttons}>
            <TouchableOpacity style={styles.cancelButton} onPress={onClose}>
              <Text>Cancel</Text>
            </TouchableOpacity>
            <TouchableOpacity
              style={[styles.submitButton, submitting && styles.disabled]}
              onPress={handleSubmit}
              disabled={submitting}
            >
              <Text style={styles.submitText}>
                {submitting ? 'Sending...' : 'Send Invitation'}
              </Text>
            </TouchableOpacity>
          </View>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  overlay: { flex: 1, backgroundColor: 'rgba(0,0,0,0.5)', justifyContent: 'center', padding: 20 },
  modal: { backgroundColor: 'white', borderRadius: 12, padding: 20 },
  title: { fontSize: 20, fontWeight: 'bold', marginBottom: 15 },
  input: { borderWidth: 1, borderColor: '#ddd', borderRadius: 8, padding: 12, marginBottom: 15 },
  messageInput: { height: 80, textAlignVertical: 'top' },
  label: { fontWeight: 'bold', marginBottom: 10 },
  studentList: { marginBottom: 15 },
  studentItem: { padding: 10, borderWidth: 1, borderColor: '#ddd', borderRadius: 8, marginBottom: 5 },
  studentSelected: { backgroundColor: '#e3f2fd', borderColor: '#2196F3' },
  buttons: { flexDirection: 'row', justifyContent: 'flex-end', gap: 10 },
  cancelButton: { padding: 12 },
  submitButton: { backgroundColor: '#007AFF', padding: 12, borderRadius: 8 },
  submitText: { color: 'white', fontWeight: 'bold' },
  disabled: { opacity: 0.6 },
});
```

### Step 3.4: Update Navigation

**File:** `mobile-apps/parent-dashboard/src/navigation/AppNavigator.tsx`

Add route for invitation acceptance:

```typescript
import InviteAcceptanceScreen from '../screens/auth/InviteAcceptanceScreen';

// In AuthStack or public routes:
<Stack.Screen
  name="AcceptInvitation"
  component={InviteAcceptanceScreen}
  options={{ title: 'Accept Invitation' }}
/>
```

### Step 3.5: Add Deep Linking Config

**File:** `mobile-apps/parent-dashboard/src/navigation/linking.ts`

```typescript
export const linking = {
  prefixes: ['gradewave://', 'https://app.gradewave.com'],
  config: {
    screens: {
      AcceptInvitation: 'invite/:token',
    },
  },
};
```

### Step 3.6: Commit

```bash
git add -A
git commit -m "feat: complete parent invitation flow frontend

- Add invitationService for API integration
- Add InviteAcceptanceScreen for parents to accept invitations
- Add InviteParentModal for school admins to send invitations
- Configure deep linking for invitation URLs
Fixes #658.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 4: Wire Shared Spelling List Components (#469, #470, #471)

**Priority:** Medium

**Current State:**
- `SharedSpellingListDetail.tsx` and `SharedSpellingListManager.tsx` exist
- Admin and Principal screens already use them
- School Admin, Teacher, Parent screens need wiring

### Step 4.1: Wire School Admin (#469)

Update school admin spelling list screens to use shared components.

### Step 4.2: Wire Teacher (#470)

Update teacher spelling list screens with classroom context.

### Step 4.3: Wire Parent (#471)

Update parent view with read-only access.

### Step 4.4: Commit

```bash
git commit -m "feat: adopt shared spelling list components across roles

- Wire school admin to SharedSpellingListManager (#469)
- Wire teacher flows with classroom context (#470)
- Wire parent read-only view (#471)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 5: Wire Shared User Detail Components (#421, #422, #423)

**Priority:** Medium

**Current State:**
- `SharedUserDetailScreen.tsx` and `SharedUserEditScreen.tsx` exist
- Adapters exist for all roles:
  - `districtUserManagementAdapter.tsx`
  - `schoolAdminUserManagementAdapter.tsx`
  - `schoolPrincipalUserManagementAdapter.tsx`
- Navigation not wired to use shared screens

### Step 5.1: Update Navigation Wrappers

Route each role's user detail flows through shared screens with appropriate adapter.

### Step 5.2: Commit

```bash
git commit -m "feat: wire shared user detail/edit screens

- Route district admin to shared screens (#421)
- Route school admin to shared screens (#422)
- Route school principal to shared screens (#423)

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Investigate Principal Dashboard Student Count (#397)

**Priority:** Medium

**Current State:** Dashboard shows 20 students but classrooms show 0 assigned.

**Investigation Steps:**
1. Check backend `/school-principal/dashboard-stats` endpoint
2. Check backend classroom enrollment queries
3. Verify data integrity in classroom_enrollments table
4. Check if student counts are aggregated correctly

This appears to be a backend/data issue rather than frontend display bug.

---

## Task 7: Verify Teacher Spelling List Filters (#396)

**Priority:** Low

**Current State:** Scope filtering exists but may not have all requested options.

**Verification:**
1. Check `getSpellingListConfig()` for teacher role
2. Verify available scopes include: All, Personal, School, Teacher, Parents
3. Add missing scopes if needed

---

## Lower Priority Tasks

### Task 8: System Sounds Management (#457)
Add UI for managing letter sounds, success/error audio.

### Task 9: Two-way Child App Notifications (#445)
Implement bidirectional notification system.

### Task 10: Integration Tests (#268-285)
Add tests for various controllers.

---

## Environment Setup

### Required Environment Variables

```bash
# Telnyx SMS (NEW)
TELNYX_API_KEY=your_api_key
TELNYX_PHONE_NUMBER=+1XXXXXXXXXX
TELNYX_MESSAGING_PROFILE_ID=optional_profile_id

# SendGrid Email (existing)
SENDGRID_API_KEY=your_api_key

# Stripe Billing (existing)
STRIPE_SECRET_KEY=your_secret_key
STRIPE_WEBHOOK_SECRET=your_webhook_secret
```

---

## Verification Checklist

- [ ] Close issues #86, #85, #88 (already fixed)
- [ ] Telnyx SMS sends successfully
- [ ] Password reset emails are sent
- [ ] Parent invitation flow works end-to-end
- [ ] Shared spelling list components work for all roles
- [ ] Shared user detail components work for all roles
- [ ] Principal dashboard counts are accurate
- [ ] All existing tests pass

---

_Plan created: 2026-01-25_
_Issues verified against current codebase_
